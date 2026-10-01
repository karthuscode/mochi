//! Minimal local consent boundary. No provider content or SQLite access.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_POLICY_BYTES: u64 = 16 * 1024;
const LOCK_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    Unavailable,
    Invalid,
    Disabled,
    Busy,
}
type Result<T> = std::result::Result<T, AuthorizationError>;

/// Native operational metadata only; intentionally not Debug-formatted.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureAuthorization {
    pub schema_version: u8,
    pub project_id: Uuid,
    pub approved_root: PathBuf,
    pub spool_root: PathBuf,
    pub policy_revision: u64,
    pub global_enabled: bool,
    pub tracking_enabled: bool,
}
impl CaptureAuthorization {
    fn validate(&self, project_id: Uuid) -> Result<()> {
        if self.schema_version != 1
            || self.project_id != project_id
            || project_id.is_nil()
            || self.policy_revision == 0
            || self.policy_revision > i64::MAX as u64
            || !self.approved_root.is_absolute()
            || !self.spool_root.is_absolute()
            || self.approved_root.as_os_str().len() > 4096
            || self.spool_root.as_os_str().len() > 4096
        {
            return Err(AuthorizationError::Invalid);
        }
        if !self.global_enabled || !self.tracking_enabled {
            return Ok(());
        }
        let root = self
            .approved_root
            .canonicalize()
            .map_err(|_| AuthorizationError::Unavailable)?;
        if root != self.approved_root
            || !root.is_dir()
            || root.parent().is_none()
            || std::env::var_os("HOME")
                .is_some_and(|home| Path::new(&home).canonicalize().ok().as_ref() == Some(&root))
        {
            return Err(AuthorizationError::Invalid);
        }
        no_links(&root)?;
        no_links(&self.spool_root)?;
        if self.spool_root.starts_with(&root) {
            return Err(AuthorizationError::Invalid);
        }
        Ok(())
    }
}

pub struct AuthorizationStore {
    root: PathBuf,
}
pub struct CaptureLease {
    _lock: File,
    policy: CaptureAuthorization,
}
impl CaptureLease {
    pub fn policy(&self) -> &CaptureAuthorization {
        &self.policy
    }
}
pub struct PolicyWriter {
    _lock: File,
    path: PathBuf,
    project_id: Uuid,
}
impl PolicyWriter {
    pub fn current_policy(&self) -> Result<CaptureAuthorization> {
        let policy = read_policy(&self.path)?;
        policy.validate(self.project_id)?;
        Ok(policy)
    }
    pub fn publish(&self, policy: &CaptureAuthorization) -> Result<()> {
        policy.validate(self.project_id)?;
        if let Ok(old) = read_policy(&self.path) {
            if old.project_id != policy.project_id || old.policy_revision > policy.policy_revision {
                return Err(AuthorizationError::Invalid);
            }
        }
        let bytes = serde_json::to_vec(policy).map_err(|_| AuthorizationError::Invalid)?;
        if bytes.len() as u64 > MAX_POLICY_BYTES {
            return Err(AuthorizationError::Invalid);
        }
        let parent = self.path.parent().ok_or(AuthorizationError::Invalid)?;
        let temp = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        let result = (|| {
            let mut file = open_private(&temp, true, true)?;
            file.write_all(&bytes)
                .map_err(|_| AuthorizationError::Unavailable)?;
            file.sync_all()
                .map_err(|_| AuthorizationError::Unavailable)?;
            fs::rename(&temp, &self.path).map_err(|_| AuthorizationError::Unavailable)?;
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|_| AuthorizationError::Unavailable)
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}
impl AuthorizationStore {
    /// Only the core creates the policy directory; helper reads never create it.
    pub fn create(root: PathBuf) -> Result<Self> {
        no_links(&root)?;
        if !root.exists() {
            let parent = root.parent().ok_or(AuthorizationError::Invalid)?;
            if !parent.is_dir() {
                return Err(AuthorizationError::Unavailable);
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&root)
                    .map_err(|_| AuthorizationError::Unavailable)?;
            }
            #[cfg(not(unix))]
            fs::create_dir(&root).map_err(|_| AuthorizationError::Unavailable)?;
        }
        Self::existing(root)
    }
    pub fn existing(root: PathBuf) -> Result<Self> {
        no_links(&root)?;
        let meta = fs::symlink_metadata(&root).map_err(|_| AuthorizationError::Unavailable)?;
        if !meta.is_dir() {
            return Err(AuthorizationError::Invalid);
        }
        private_metadata(&meta, 0o700)?;
        Ok(Self { root })
    }
    pub fn authorize(&self, id: Uuid) -> Result<CaptureLease> {
        let lock = open_private(&self.root.join(format!("{id}.lock")), false, false)?;
        acquire(&lock, false)?;
        let policy = read_policy(&self.root.join(format!("{id}.json")))?;
        policy.validate(id)?;
        if !policy.global_enabled || !policy.tracking_enabled {
            return Err(AuthorizationError::Disabled);
        }
        Ok(CaptureLease {
            _lock: lock,
            policy,
        })
    }
    pub fn writer(&self, id: Uuid) -> Result<PolicyWriter> {
        if id.is_nil() {
            return Err(AuthorizationError::Invalid);
        }
        let lock = open_private(&self.root.join(format!("{id}.lock")), true, false)?;
        acquire(&lock, true)?;
        Ok(PolicyWriter {
            _lock: lock,
            path: self.root.join(format!("{id}.json")),
            project_id: id,
        })
    }
}
fn acquire(file: &File, exclusive: bool) -> Result<()> {
    let started = Instant::now();
    loop {
        let result = if exclusive {
            FileExt::try_lock_exclusive(file)
        } else {
            FileExt::try_lock_shared(file)
        };
        match result {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() >= LOCK_TIMEOUT {
                    return Err(AuthorizationError::Busy);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => return Err(AuthorizationError::Unavailable),
        }
    }
}
fn read_policy(path: &Path) -> Result<CaptureAuthorization> {
    let file = open_private(path, false, false)?;
    if file
        .metadata()
        .map_err(|_| AuthorizationError::Unavailable)?
        .len()
        > MAX_POLICY_BYTES
    {
        return Err(AuthorizationError::Invalid);
    }
    let mut bytes = Vec::new();
    file.take(MAX_POLICY_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AuthorizationError::Unavailable)?;
    if bytes.len() as u64 > MAX_POLICY_BYTES {
        return Err(AuthorizationError::Invalid);
    }
    serde_json::from_slice(&bytes).map_err(|_| AuthorizationError::Invalid)
}
fn open_private(path: &Path, create: bool, create_new: bool) -> Result<File> {
    no_links(path)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(create)
        .create(create && !create_new)
        .create_new(create_new);
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options
        .open(path)
        .map_err(|_| AuthorizationError::Unavailable)?;
    let meta = file
        .metadata()
        .map_err(|_| AuthorizationError::Unavailable)?;
    if !meta.is_file() {
        return Err(AuthorizationError::Invalid);
    }
    private_metadata(&meta, 0o600)?;
    Ok(file)
}
fn private_metadata(meta: &fs::Metadata, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        // Read-only OS identity query; it neither reads credentials nor mutates state.
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o777 != mode {
            return Err(AuthorizationError::Invalid);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (meta, mode);
        return Err(AuthorizationError::Unavailable);
    }
    Ok(())
}
fn no_links(path: &Path) -> Result<()> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(AuthorizationError::Invalid);
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(m) if m.file_type().is_symlink() => return Err(AuthorizationError::Invalid),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AuthorizationError::Unavailable),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    fn fixture() -> (tempfile::TempDir, AuthorizationStore, CaptureAuthorization) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("project")).unwrap();
        let policy = CaptureAuthorization {
            schema_version: 1,
            project_id: Uuid::new_v4(),
            approved_root: root.join("project"),
            spool_root: root.join("spool"),
            policy_revision: 1,
            global_enabled: true,
            tracking_enabled: true,
        };
        let store = AuthorizationStore::create(root.join("policies")).unwrap();
        (temp, store, policy)
    }
    #[test]
    fn missing_disabled_and_global_off_fail_closed() {
        let (_temp, store, mut p) = fixture();
        assert!(store.authorize(p.project_id).is_err());
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        assert_eq!(
            store
                .authorize(p.project_id)
                .unwrap()
                .policy()
                .policy_revision,
            1
        );
        p.global_enabled = false;
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        assert!(matches!(
            store.authorize(p.project_id),
            Err(AuthorizationError::Disabled)
        ));
        p.global_enabled = true;
        p.tracking_enabled = false;
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        assert!(store.authorize(p.project_id).is_err());
    }
    #[test]
    fn revision_regression_corruption_and_identity_fail_closed() {
        let (_temp, store, mut p) = fixture();
        p.policy_revision = 2;
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        p.policy_revision = 1;
        assert!(store.writer(p.project_id).unwrap().publish(&p).is_err());
        fs::write(
            store.root.join(format!("{}.json", p.project_id)),
            b"{malformed}",
        )
        .unwrap();
        assert!(store.authorize(p.project_id).is_err());
        p.policy_revision = 3;
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        let other = Uuid::new_v4();
        fs::copy(
            store.root.join(format!("{}.json", p.project_id)),
            store.root.join(format!("{other}.json")),
        )
        .unwrap();
        drop(store.writer(other).unwrap());
        assert!(store.authorize(other).is_err());
    }
    #[test]
    fn revocation_serializes_and_contention_is_bounded() {
        let (_temp, store, mut p) = fixture();
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        let lease = store.authorize(p.project_id).unwrap();
        let start = Instant::now();
        assert!(matches!(
            store.writer(p.project_id),
            Err(AuthorizationError::Busy)
        ));
        assert!(start.elapsed() < Duration::from_secs(2));
        drop(lease);
        p.policy_revision = 2;
        p.tracking_enabled = false;
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        assert!(store.authorize(p.project_id).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn permissions_and_symlinks_are_rejected() {
        let (_temp, store, p) = fixture();
        {
            store.writer(p.project_id).unwrap().publish(&p).unwrap();
        }
        let path = store.root.join(format!("{}.json", p.project_id));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.authorize(p.project_id).is_err());
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&p.approved_root, &path).unwrap();
        assert!(store.authorize(p.project_id).is_err());
    }
}
