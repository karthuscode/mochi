//! Consented, project-scoped hook edits. Plans are memory-only; receipts contain
//! only Mochi-owned operational metadata, never arbitrary user configuration.
use crate::hooks::{parse_config, read_config, shell_quote, CAPTURE_EVENTS, MAX_CONFIG_BYTES};
use crate::process::run_bounded;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallError {
    InvalidRequest,
    UnsafePath,
    Unreadable,
    MalformedConfiguration,
    UnsupportedConfiguration,
    HelperUnavailable,
    ApprovalRequired,
    ConcurrentEdit,
    OwnershipConflict,
    Busy,
    WriteFailed,
    RecoveryRequired,
    NotInstalled,
}
impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "integration: {self:?}")
    }
}
impl std::error::Error for InstallError {}
type Result<T> = std::result::Result<T, InstallError>;

/// Native-only approved project metadata. Neither detection nor a preview grants
/// root access or local tracking consent. The caller must obtain both first.
#[derive(Clone)]
pub struct InstallRequest {
    pub project_id: Uuid,
    pub approved_root: PathBuf,
    pub helper_path: PathBuf,
    pub spool_root: PathBuf,
    pub policy_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallAction {
    Install,
    Upgrade,
    Disconnect,
    Rollback,
}

/// Explicit local preview only. No preexisting commands/configuration are exposed.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPreview {
    pub schema_version: u8,
    pub plan_id: Uuid,
    pub action: InstallAction,
    pub target: PathBuf,
    pub owned_command: String,
    pub events: Vec<String>,
    pub changes_configuration: bool,
    pub requires_codex_trust_review: bool,
    pub backup_retention_days: u8,
}

pub struct InstallPlan {
    preview: InstallPreview,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
    receipt: Receipt,
    next: Option<OwnedHooks>,
    helper: Option<(PathBuf, String)>,
}
impl InstallPlan {
    pub fn preview(&self) -> &InstallPreview {
        &self.preview
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallOutcome {
    Changed,
    Unchanged,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnedHooks {
    command: String,
    helper_hash: String,
    created_file: bool,
    created_events: BTreeMap<String, bool>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pending {
    before_hash: String,
    after_hash: String,
    next: Option<OwnedHooks>,
    previous: Option<OwnedHooks>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: u8,
    target: PathBuf,
    current: Option<OwnedHooks>,
    previous: Option<OwnedHooks>,
    pending: Option<Pending>,
}

pub struct CodexInstaller {
    state_root: PathBuf,
}
impl CodexInstaller {
    pub fn new(state_root: impl AsRef<Path>) -> Result<Self> {
        let state_root = state_root.as_ref();
        if !state_root.is_absolute() {
            return Err(InstallError::InvalidRequest);
        }
        private_directory(state_root)?;
        #[cfg(unix)]
        fs::set_permissions(state_root, fs::Permissions::from_mode(0o700))
            .map_err(|_| InstallError::WriteFailed)?;
        let installer = Self {
            state_root: state_root
                .canonicalize()
                .map_err(|_| InstallError::UnsafePath)?,
        };
        private_directory(&installer.state_root.join("backups"))?;
        #[cfg(unix)]
        fs::set_permissions(
            installer.state_root.join("backups"),
            fs::Permissions::from_mode(0o700),
        )
        .map_err(|_| InstallError::WriteFailed)?;
        installer.cleanup_backups(SystemTime::now())?;
        Ok(installer)
    }

    pub fn prepare_install(&self, request: InstallRequest) -> Result<InstallPlan> {
        self.prepare_install_mode(request, None)
    }

    /// Product capture reads current consent on every invocation. This preview is
    /// still not consent; the core must publish the approved policy separately.
    pub fn prepare_authorized_install(
        &self,
        request: InstallRequest,
        policy_root: &Path,
    ) -> Result<InstallPlan> {
        if !policy_root.is_absolute() {
            return Err(InstallError::InvalidRequest);
        }
        check_path(policy_root)?;
        self.prepare_install_mode(request, Some(policy_root))
    }

    fn prepare_install_mode(
        &self,
        request: InstallRequest,
        policy_root: Option<&Path>,
    ) -> Result<InstallPlan> {
        if request.project_id.is_nil()
            || request.policy_revision == 0
            || !request.approved_root.is_absolute()
            || !request.helper_path.is_absolute()
        {
            return Err(InstallError::InvalidRequest);
        }
        let root = request
            .approved_root
            .canonicalize()
            .map_err(|_| InstallError::UnsafePath)?;
        if root.parent().is_none()
            || !root.is_dir()
            || std::env::var_os("HOME")
                .is_some_and(|home| Path::new(&home).canonicalize().ok().as_ref() == Some(&root))
        {
            return Err(InstallError::UnsafePath);
        }
        check_path(&request.approved_root)?;
        check_path(&root)?;
        let target = root.join(".codex/hooks.json");
        check_path(&target)?;
        check_path(&request.helper_path)?;
        let helper = request
            .helper_path
            .canonicalize()
            .map_err(|_| InstallError::HelperUnavailable)?;
        check_path(&helper)?;
        let helper_hash = validate_helper(&helper)?;
        if !request.spool_root.is_absolute() {
            return Err(InstallError::InvalidRequest);
        }
        check_path(&request.spool_root)?;
        let command = if let Some(policy_root) = policy_root {
            format!(
                "{} capture-authorized --project-id {} --policy-root {}",
                quote_path(&helper)?,
                request.project_id,
                quote_path(policy_root)?
            )
        } else {
            format!("{} capture --project-id {} --approved-root {} --client-surface cli --policy-revision {} --spool-root {}", quote_path(&helper)?, request.project_id, quote_path(&root)?, request.policy_revision, quote_path(&request.spool_root)?)
        };
        let _lock = self.lock(&target)?;
        let before = load_target(&target)?;
        let receipt = self.receipt(&target, &before)?;
        let mut document = document(&before)?;
        reject_inline_hooks(&target)?;
        let action = if receipt.current.is_some() {
            InstallAction::Upgrade
        } else {
            InstallAction::Install
        };
        if let Some(owned) = &receipt.current {
            remove_owned(&mut document, owned)?;
        }
        reject_unowned_mochi(&document)?;
        let created_events = add_owned(&mut document, &command)?;
        let next = OwnedHooks {
            command: command.clone(),
            helper_hash: helper_hash.clone(),
            created_file: receipt
                .current
                .as_ref()
                .map_or(before.is_none(), |v| v.created_file),
            created_events: receipt
                .current
                .as_ref()
                .map_or(created_events, |v| v.created_events.clone()),
        };
        let unchanged = receipt
            .current
            .as_ref()
            .is_some_and(|v| v.command == command && v.helper_hash == helper_hash);
        let after = if unchanged {
            before.clone()
        } else {
            Some(render_hooks(&before, &document)?)
        };
        Ok(self.plan(
            action,
            target,
            before,
            after,
            receipt,
            Some(next),
            Some((helper, helper_hash)),
            command,
        ))
    }

    pub fn prepare_disconnect(&self, approved_root: &Path) -> Result<InstallPlan> {
        self.prepare_removal(approved_root, false)
    }
    pub fn prepare_rollback(&self, approved_root: &Path) -> Result<InstallPlan> {
        self.prepare_removal(approved_root, true)
    }
    fn prepare_removal(&self, approved_root: &Path, rollback: bool) -> Result<InstallPlan> {
        check_path(approved_root)?;
        let target = approved_root
            .canonicalize()
            .map_err(|_| InstallError::UnsafePath)?
            .join(".codex/hooks.json");
        check_path(&target)?;
        let _lock = self.lock(&target)?;
        let before = load_target(&target)?;
        let receipt = self.receipt(&target, &before)?;
        let owned = receipt.current.as_ref().ok_or(InstallError::NotInstalled)?;
        let mut document = document(&before)?;
        remove_owned(&mut document, owned)?;
        let next = if rollback {
            receipt.previous.clone()
        } else {
            None
        };
        if let Some(previous) = &next {
            add_owned(&mut document, &previous.command)?;
        }
        let only_empty_hooks = document.as_object().is_some_and(|v| {
            v.iter().all(|(key, value)| {
                key == "hooks" && value.as_object().is_some_and(|v| v.is_empty())
            })
        });
        let after = if owned.created_file && next.is_none() && only_empty_hooks {
            None
        } else {
            Some(render_hooks(&before, &document)?)
        };
        let command = next
            .as_ref()
            .map_or_else(|| owned.command.clone(), |v| v.command.clone());
        Ok(self.plan(
            if rollback {
                InstallAction::Rollback
            } else {
                InstallAction::Disconnect
            },
            target,
            before,
            after,
            receipt,
            next,
            None,
            command,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn plan(
        &self,
        action: InstallAction,
        target: PathBuf,
        before: Option<Vec<u8>>,
        after: Option<Vec<u8>>,
        receipt: Receipt,
        next: Option<OwnedHooks>,
        helper: Option<(PathBuf, String)>,
        command: String,
    ) -> InstallPlan {
        let changed = before != after;
        InstallPlan {
            preview: InstallPreview {
                schema_version: 1,
                plan_id: Uuid::new_v4(),
                action,
                target,
                owned_command: command,
                events: CAPTURE_EVENTS.iter().map(|v| (*v).to_owned()).collect(),
                changes_configuration: changed,
                requires_codex_trust_review: next.is_some() && (changed || helper.is_some()),
                backup_retention_days: 7,
            },
            before,
            after,
            receipt,
            next,
            helper,
        }
    }

    /// Call only following explicit approval of this exact preview. No automatic
    /// startup installation, trust mutation, integration test, or capture occurs.
    pub fn apply(&self, plan: InstallPlan, approved_plan_id: Uuid) -> Result<InstallOutcome> {
        if approved_plan_id != plan.preview.plan_id {
            return Err(InstallError::ApprovalRequired);
        }
        let target = &plan.preview.target;
        let _lock = self.lock(target)?;
        check_path(target)?;
        let current = load_target(target)?;
        if digest(&current) != digest(&plan.before) {
            return Err(InstallError::ConcurrentEdit);
        }
        let receipt = self.receipt(target, &current)?;
        if serde_json::to_vec(&receipt).map_err(|_| InstallError::RecoveryRequired)?
            != serde_json::to_vec(&plan.receipt).map_err(|_| InstallError::RecoveryRequired)?
        {
            return Err(InstallError::ConcurrentEdit);
        }
        if let Some((helper, expected)) = &plan.helper {
            if validate_helper(helper)? != *expected {
                return Err(InstallError::ConcurrentEdit);
            }
            reject_inline_hooks(target)?;
        }
        if !plan.preview.changes_configuration {
            // A rebuilt compatible helper can have unchanged arguments. Record
            // its new fingerprint without rewriting identical hook bytes.
            let mut updated = receipt;
            updated.current = plan.next;
            self.save_receipt(&updated)?;
            return Ok(InstallOutcome::Unchanged);
        }
        private_directory(target.parent().ok_or(InstallError::UnsafePath)?)?;
        if let Some(bytes) = &current {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| InstallError::WriteFailed)?
                .as_secs();
            write_atomic(
                &self
                    .state_root
                    .join("backups")
                    .join(format!("{now}-{}.json", Uuid::new_v4())),
                bytes,
            )?;
        }
        let mut journal = receipt;
        journal.pending = Some(Pending {
            before_hash: digest(&plan.before),
            after_hash: digest(&plan.after),
            next: plan.next.clone(),
            previous: journal.current.clone(),
        });
        self.save_receipt(&journal)?;
        // Recheck immediately before replacement. Other Mochi callers share the
        // lock; unrelated editors are detected by the content precondition.
        if digest(&load_target(target)?) != digest(&plan.before) {
            return Err(InstallError::ConcurrentEdit);
        }
        match &plan.after {
            Some(bytes) => write_atomic(target, bytes)?,
            None => {
                fs::remove_file(target).map_err(|_| InstallError::WriteFailed)?;
                sync_parent(target)?;
            }
        }
        if digest(&load_target(target)?) != digest(&plan.after) {
            return Err(InstallError::RecoveryRequired);
        }
        journal.previous = if plan.preview.action == InstallAction::Disconnect {
            None
        } else {
            journal.current.take()
        };
        journal.current = plan.next;
        journal.pending = None;
        self.save_receipt(&journal)
            .map_err(|_| InstallError::RecoveryRequired)?;
        Ok(InstallOutcome::Changed)
    }

    fn receipt(&self, target: &Path, content: &Option<Vec<u8>>) -> Result<Receipt> {
        let path = self.record_path(target);
        let mut receipt = match load_target(&path)? {
            Some(bytes) => serde_json::from_slice::<Receipt>(&bytes)
                .map_err(|_| InstallError::RecoveryRequired)?,
            None => Receipt {
                schema_version: 1,
                target: target.to_path_buf(),
                current: None,
                previous: None,
                pending: None,
            },
        };
        if receipt.schema_version != 1 || receipt.target != target {
            return Err(InstallError::RecoveryRequired);
        }
        if let Some(pending) = receipt.pending.take() {
            if digest(content) == pending.after_hash {
                receipt.current = pending.next;
                receipt.previous = pending.previous;
            } else if digest(content) != pending.before_hash {
                return Err(InstallError::RecoveryRequired);
            }
            self.save_receipt(&receipt)?;
        }
        Ok(receipt)
    }
    fn record_path(&self, target: &Path) -> PathBuf {
        self.state_root.join(format!(
            "{}.json",
            hash(target.as_os_str().as_encoded_bytes())
        ))
    }
    fn save_receipt(&self, receipt: &Receipt) -> Result<()> {
        let bytes = serde_json::to_vec(receipt).map_err(|_| InstallError::WriteFailed)?;
        write_atomic(&self.record_path(&receipt.target), &bytes)
    }
    fn lock(&self, target: &Path) -> Result<File> {
        let path = self.record_path(target).with_extension("lock");
        check_path(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = options.open(path).map_err(|_| InstallError::Unreadable)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| InstallError::WriteFailed)?;
        let started = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(file),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && started.elapsed() < Duration::from_secs(5) =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(InstallError::Busy)
                }
                Err(_) => return Err(InstallError::Unreadable),
            }
        }
    }

    /// Explicit maintenance and constructor-time cleanup; no background scheduler.
    /// Backup names carry only creation time and an opaque identifier.
    pub fn cleanup_backups(&self, now: SystemTime) -> Result<usize> {
        let now = now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| InstallError::InvalidRequest)?
            .as_secs();
        let mut removed = 0;
        let entries =
            fs::read_dir(self.state_root.join("backups")).map_err(|_| InstallError::Unreadable)?;
        for entry in entries.take(1024) {
            let entry = entry.map_err(|_| InstallError::Unreadable)?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((timestamp, id)) = name.strip_suffix(".json").and_then(|v| v.split_once('-'))
            else {
                continue;
            };
            let Ok(timestamp) = timestamp.parse::<u64>() else {
                continue;
            };
            if Uuid::parse_str(id).is_err() || now.saturating_sub(timestamp) < 7 * 86400 {
                if Uuid::parse_str(id).is_ok() {
                    check_path(&entry.path())?;
                    #[cfg(unix)]
                    fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o600))
                        .map_err(|_| InstallError::WriteFailed)?;
                }
                continue;
            }
            check_path(&entry.path())?;
            fs::remove_file(entry.path()).map_err(|_| InstallError::WriteFailed)?;
            removed += 1;
        }
        Ok(removed)
    }
}

fn quote_path(path: &Path) -> Result<String> {
    shell_quote(path.to_str().ok_or(InstallError::UnsafePath)?)
        .map_err(|_| InstallError::UnsafePath)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(bytes: &Option<Vec<u8>>) -> String {
    match bytes {
        Some(v) => format!("present:{}", hash(v)),
        None => "absent".to_owned(),
    }
}
fn load_target(path: &Path) -> Result<Option<Vec<u8>>> {
    check_path(path)?;
    read_config(path).map_err(|_| InstallError::Unreadable)
}
fn document(bytes: &Option<Vec<u8>>) -> Result<Value> {
    match bytes {
        Some(v) => parse_config(v).map_err(|_| InstallError::MalformedConfiguration),
        None => Ok(json!({})),
    }
}
fn handler(command: &str) -> Value {
    json!({"type": "command", "command": command, "timeout": 3})
}
fn add_owned(document: &mut Value, command: &str) -> Result<BTreeMap<String, bool>> {
    let root = document
        .as_object_mut()
        .ok_or(InstallError::MalformedConfiguration)?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or(InstallError::MalformedConfiguration)?;
    let mut created = BTreeMap::new();
    for event in CAPTURE_EVENTS {
        created.insert(event.to_owned(), !hooks.contains_key(event));
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or(InstallError::MalformedConfiguration)?;
        if groups.len() >= 256 {
            return Err(InstallError::UnsupportedConfiguration);
        }
        groups.push(json!({"hooks": [handler(command)]}));
    }
    Ok(created)
}
fn remove_owned(document: &mut Value, owned: &OwnedHooks) -> Result<()> {
    let hooks = document
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or(InstallError::OwnershipConflict)?;
    let expected = handler(&owned.command);
    for event in CAPTURE_EVENTS {
        let groups = hooks
            .get_mut(event)
            .and_then(Value::as_array_mut)
            .ok_or(InstallError::OwnershipConflict)?;
        let mut removed = 0;
        for group in groups.iter_mut() {
            let handlers = group
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .ok_or(InstallError::OwnershipConflict)?;
            for value in handlers.iter() {
                if value.get("command").and_then(Value::as_str) == Some(&owned.command)
                    && *value != expected
                {
                    return Err(InstallError::OwnershipConflict);
                }
            }
            handlers.retain(|value| {
                if *value == expected {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
        if removed != 1 {
            return Err(InstallError::OwnershipConflict);
        }
        groups.retain(|group| {
            !group.as_object().is_some_and(|v| {
                v.len() == 1
                    && v.get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty)
            })
        });
        if groups.is_empty() && owned.created_events.get(event) == Some(&true) {
            hooks.remove(event);
        }
    }
    Ok(())
}
fn reject_unowned_mochi(document: &Value) -> Result<()> {
    if let Some(events) = document.get("hooks").and_then(Value::as_object) {
        for groups in events.values() {
            for group in groups.as_array().into_iter().flatten() {
                for handler in group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if handler
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.contains("mochi-hook"))
                    {
                        return Err(InstallError::OwnershipConflict);
                    }
                }
            }
        }
    }
    Ok(())
}
fn reject_inline_hooks(target: &Path) -> Result<()> {
    let toml = target.with_file_name("config.toml");
    if let Some(bytes) = load_target(&toml)? {
        let text =
            std::str::from_utf8(&bytes).map_err(|_| InstallError::UnsupportedConfiguration)?;
        let document = text
            .parse::<toml::Value>()
            .map_err(|_| InstallError::UnsupportedConfiguration)?;
        if document.get("hooks").is_some() {
            return Err(InstallError::UnsupportedConfiguration);
        }
    }
    Ok(())
}

/// Replace only the hooks value. All non-hook bytes, including unrelated secrets
/// and formatting, remain unchanged and are never returned by the public API.
fn render_hooks(before: &Option<Vec<u8>>, document: &Value) -> Result<Vec<u8>> {
    #[derive(Deserialize)]
    struct Raw<'a> {
        #[serde(borrow)]
        hooks: Option<&'a serde_json::value::RawValue>,
    }
    let hooks = document
        .get("hooks")
        .ok_or(InstallError::MalformedConfiguration)?;
    let replacement = serde_json::to_vec_pretty(hooks).map_err(|_| InstallError::WriteFailed)?;
    let bytes = if let Some(before) = before {
        let raw: Raw<'_> =
            serde_json::from_slice(before).map_err(|_| InstallError::MalformedConfiguration)?;
        if let Some(raw) = raw.hooks {
            let start = raw.get().as_ptr() as usize - before.as_ptr() as usize;
            let end = start + raw.get().len();
            let mut result = before[..start].to_vec();
            result.extend(&replacement);
            result.extend(&before[end..]);
            result
        } else {
            let end = before
                .iter()
                .rposition(|v| *v == b'}')
                .ok_or(InstallError::MalformedConfiguration)?;
            let original: Value =
                serde_json::from_slice(before).map_err(|_| InstallError::MalformedConfiguration)?;
            let mut result = before[..end].to_vec();
            if original.as_object().is_some_and(|v| !v.is_empty()) {
                result.push(b',');
            }
            result.extend(b"\n\"hooks\": ");
            result.extend(&replacement);
            result.extend(&before[end..]);
            result
        }
    } else {
        serde_json::to_vec_pretty(document).map_err(|_| InstallError::WriteFailed)?
    };
    if bytes.len() as u64 > MAX_CONFIG_BYTES || parse_config(&bytes).is_err() {
        return Err(InstallError::MalformedConfiguration);
    }
    Ok(bytes)
}
fn validate_helper(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path).map_err(|_| InstallError::HelperUnavailable)?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(InstallError::HelperUnavailable);
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(InstallError::HelperUnavailable);
    }
    let version = run_bounded(path, &["version"], Duration::from_secs(2))
        .map_err(|_| InstallError::HelperUnavailable)?;
    if version.trim() != format!("mochi-hook {}", env!("CARGO_PKG_VERSION")) {
        return Err(InstallError::HelperUnavailable);
    }
    check_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options
        .open(path)
        .map_err(|_| InstallError::HelperUnavailable)?;
    if !file
        .metadata()
        .map_err(|_| InstallError::HelperUnavailable)?
        .is_file()
    {
        return Err(InstallError::HelperUnavailable);
    }
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| InstallError::HelperUnavailable)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(InstallError::HelperUnavailable);
    }
    Ok(hash(&bytes))
}
fn check_path(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|v| matches!(v, std::path::Component::ParentDir))
    {
        return Err(InstallError::UnsafePath);
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(InstallError::UnsafePath)
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(InstallError::UnsafePath),
        }
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        #[cfg(unix)]
        // SAFETY: geteuid has no arguments and cannot fail.
        if metadata.uid() != unsafe { libc::geteuid() } {
            return Err(InstallError::UnsafePath);
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(InstallError::UnsafePath);
        }
    }
    Ok(())
}
fn private_directory(path: &Path) -> Result<()> {
    check_path(path)?;
    let existed = path.exists();
    fs::create_dir_all(path).map_err(|_| InstallError::WriteFailed)?;
    #[cfg(unix)]
    if !existed {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|_| InstallError::WriteFailed)?;
    }
    if !path.is_dir() {
        return Err(InstallError::UnsafePath);
    }
    Ok(())
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    check_path(path)?;
    let parent = path.parent().ok_or(InstallError::UnsafePath)?;
    let temporary = parent.join(format!(".mochi-{}", Uuid::new_v4()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let mut file = options
            .open(&temporary)
            .map_err(|_| InstallError::WriteFailed)?;
        file.write_all(bytes)
            .map_err(|_| InstallError::WriteFailed)?;
        file.sync_all().map_err(|_| InstallError::WriteFailed)?;
        check_path(path)?;
        fs::rename(&temporary, path).map_err(|_| InstallError::WriteFailed)?;
        sync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn sync_parent(path: &Path) -> Result<()> {
    File::open(path.parent().ok_or(InstallError::UnsafePath)?)
        .and_then(|file| file.sync_all())
        .map_err(|_| InstallError::WriteFailed)
}

#[cfg(test)]
mod tests;
