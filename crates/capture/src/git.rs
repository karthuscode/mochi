use crate::sanitize::{CaptureSanitizer, ContentSanitizer};
use mochi_privacy::{Decision, FilePolicy, Reason, RedactionEngine, Source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub const MAX_GIT_FILES: usize = 100;
pub const MAX_GIT_FILE_BYTES: u64 = 64 * 1024;
pub const MAX_GIT_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_GIT_COMMAND_BYTES: usize = 2 * 1024 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotRole {
    Baseline,
    Final,
}

impl SnapshotRole {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "baseline" => Some(Self::Baseline),
            "final" => Some(Self::Final),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitSnapshot {
    pub schema_version: u8,
    pub project_id: Uuid,
    pub role: SnapshotRole,
    pub captured_at: String,
    pub repository_root: String,
    pub branch: Option<String>,
    pub detached: bool,
    pub head: Option<String>,
    pub files: Vec<GitFileSnapshot>,
    pub excluded_count: u64,
    pub omitted_file_count: u64,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitFileSnapshot {
    pub path: String,
    pub old_path: Option<String>,
    pub change: GitChange,
    pub staged: bool,
    pub unstaged: bool,
    pub size_bytes: Option<u64>,
    pub binary: bool,
    pub content_hash: Option<String>,
    pub content: Option<String>,
    pub omission: Option<GitContentOmission>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitChange {
    Clean,
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitContentOmission {
    Deleted,
    Binary,
    SizeLimit,
    AggregateLimit,
    Symlink,
    Unreadable,
    Policy,
}

#[derive(Debug)]
pub enum GitError {
    NotRepository,
    RootMismatch,
    CommandFailed,
    Timeout,
    OutputLimit,
    InvalidOutput,
    Io,
    Sanitization,
}

pub trait GitContextReader {
    fn snapshot(
        &self,
        project_id: Uuid,
        approved_root: &Path,
        role: SnapshotRole,
    ) -> Result<GitSnapshot, GitError>;
}

pub struct GitCliContextReader;

#[derive(Clone, Debug, Default)]
struct StatusEntry {
    change: Option<GitChange>,
    staged: bool,
    unstaged: bool,
    old_path: Option<String>,
}

impl GitContextReader for GitCliContextReader {
    fn snapshot(
        &self,
        project_id: Uuid,
        approved_root: &Path,
        role: SnapshotRole,
    ) -> Result<GitSnapshot, GitError> {
        let approved_root = approved_root.canonicalize().map_err(|_| GitError::Io)?;
        let discovered = run_git(&approved_root, &["rev-parse", "--show-toplevel"])?;
        let discovered = String::from_utf8(discovered).map_err(|_| GitError::InvalidOutput)?;
        let discovered = PathBuf::from(discovered.trim())
            .canonicalize()
            .map_err(|_| GitError::InvalidOutput)?;
        if discovered != approved_root {
            return Err(GitError::RootMismatch);
        }

        let branch = run_git_optional(&approved_root, &["symbolic-ref", "--short", "-q", "HEAD"])?
            .map(|value| String::from_utf8(value).map_err(|_| GitError::InvalidOutput))
            .transpose()?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let head = run_git_optional(&approved_root, &["rev-parse", "--verify", "HEAD"])?
            .map(|value| String::from_utf8(value).map_err(|_| GitError::InvalidOutput))
            .transpose()?
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        let status_output = run_git(
            &approved_root,
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        )?;
        let statuses = parse_status(&status_output)?;
        let tracked = parse_nul_paths(&run_git(&approved_root, &["ls-files", "-z"])?);
        let untracked = parse_nul_paths(&run_git(
            &approved_root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?);
        let mut paths = tracked.into_iter().collect::<BTreeSet<_>>();
        paths.extend(untracked);

        let sanitizer =
            CaptureSanitizer::new(&approved_root).map_err(|_| GitError::Sanitization)?;
        let redactor = RedactionEngine::new().map_err(|_| GitError::Sanitization)?;
        let policy = FilePolicy::load(&approved_root).map_err(|_| GitError::Sanitization)?;
        let mut files = Vec::new();
        let mut excluded_count = 0u64;
        let mut omitted_file_count = 0u64;
        let mut aggregate_bytes = 0usize;
        let mut truncated = false;

        for relative in paths {
            let decision = policy.evaluate(Path::new(&relative), None, Source::Git);
            if decision.decision == Decision::Deny {
                excluded_count = excluded_count.saturating_add(1);
                continue;
            }
            let status = statuses.get(&relative).cloned().unwrap_or_default();
            if status.old_path.as_ref().is_some_and(|old| {
                policy.evaluate(Path::new(old), None, Source::Git).decision == Decision::Deny
            }) {
                excluded_count = excluded_count.saturating_add(1);
                continue;
            }
            if files.len() >= MAX_GIT_FILES {
                omitted_file_count = omitted_file_count.saturating_add(1);
                truncated = true;
                continue;
            }
            let Some(safe_path) = decision.safe_path.as_ref() else {
                return Err(GitError::InvalidOutput);
            };
            if redactor
                .redact_text(safe_path)
                .map_err(|_| GitError::Sanitization)?
                .changed
                || status.old_path.as_ref().is_some_and(|old| {
                    redactor
                        .redact_text(old)
                        .map(|result| result.changed)
                        .unwrap_or(true)
                })
            {
                excluded_count = excluded_count.saturating_add(1);
                continue;
            }
            let path = approved_root.join(safe_path);
            let mut file = GitFileSnapshot {
                path: safe_path.clone(),
                old_path: status.old_path,
                change: status.change.unwrap_or(GitChange::Clean),
                staged: status.staged,
                unstaged: status.unstaged,
                size_bytes: None,
                binary: false,
                content_hash: None,
                content: None,
                omission: None,
            };

            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) if file.change == GitChange::Deleted => {
                    file.omission = Some(GitContentOmission::Deleted);
                    files.push(file);
                    continue;
                }
                Err(_) => {
                    file.omission = Some(GitContentOmission::Unreadable);
                    files.push(file);
                    continue;
                }
            };
            file.size_bytes = Some(metadata.len());
            let decision = policy.evaluate(Path::new(&relative), Some(metadata.len()), Source::Git);
            if decision.decision == Decision::Deny {
                excluded_count = excluded_count.saturating_add(1);
                continue;
            }
            if decision.decision == Decision::MetadataOnly {
                file.omission = Some(if decision.reason == Reason::FileTooLarge {
                    GitContentOmission::SizeLimit
                } else if decision.reason == Reason::BinaryFile {
                    GitContentOmission::Binary
                } else {
                    GitContentOmission::Policy
                });
                files.push(file);
                continue;
            }
            if metadata.file_type().is_symlink() {
                file.omission = Some(GitContentOmission::Symlink);
                files.push(file);
                continue;
            }
            if !metadata.is_file() {
                file.omission = Some(GitContentOmission::Unreadable);
                files.push(file);
                continue;
            }
            if metadata.len() > MAX_GIT_FILE_BYTES {
                file.omission = Some(GitContentOmission::SizeLimit);
                truncated = true;
                files.push(file);
                continue;
            }
            let Some(bytes) = read_verified_file(&approved_root, &path)? else {
                file.omission = Some(GitContentOmission::SizeLimit);
                truncated = true;
                files.push(file);
                continue;
            };
            if policy
                .classify_sample(Path::new(&relative), &bytes, Source::Git)
                .decision
                != Decision::Allow
            {
                file.binary = true;
                file.omission = Some(GitContentOmission::Binary);
                files.push(file);
                continue;
            }
            let raw = std::str::from_utf8(&bytes).map_err(|_| GitError::InvalidOutput)?;
            let sanitized = sanitizer
                .sanitize_text(raw)
                .map_err(|_| GitError::Sanitization)?;
            let projected = aggregate_bytes.saturating_add(sanitized.value.len());
            if projected > MAX_GIT_SNAPSHOT_BYTES {
                file.omission = Some(GitContentOmission::AggregateLimit);
                truncated = true;
                files.push(file);
                continue;
            }
            aggregate_bytes = projected;
            file.content_hash = Some(hex_sha256(sanitized.value.as_bytes()));
            file.content = Some(sanitized.value);
            files.push(file);
        }

        let branch = branch.and_then(|value| match redactor.redact_text(&value) {
            Ok(result) if !result.changed => Some(value),
            _ => None,
        });
        Ok(GitSnapshot {
            schema_version: 1,
            project_id,
            role,
            captured_at: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .map_err(|_| GitError::InvalidOutput)?,
            repository_root: "[PROJECT_ROOT]".to_owned(),
            branch,
            detached: head.is_some()
                && run_git_optional(&approved_root, &["symbolic-ref", "-q", "HEAD"])?.is_none(),
            head,
            files,
            excluded_count,
            omitted_file_count,
            truncated,
        })
    }
}

fn read_verified_file(root: &Path, path: &Path) -> Result<Option<Vec<u8>>, GitError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| GitError::Io)?;
    let opened = file.metadata().map_err(|_| GitError::Io)?;
    if !opened.is_file() {
        return Err(GitError::InvalidOutput);
    }
    let current = path.canonicalize().map_err(|_| GitError::Io)?;
    if !current.starts_with(root)
        || std::fs::symlink_metadata(path)
            .map_err(|_| GitError::Io)?
            .file_type()
            .is_symlink()
    {
        return Err(GitError::RootMismatch);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let resolved = std::fs::metadata(&current).map_err(|_| GitError::Io)?;
        if opened.dev() != resolved.dev() || opened.ino() != resolved.ino() {
            return Err(GitError::RootMismatch);
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_GIT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| GitError::Io)?;
    if bytes.len() as u64 > MAX_GIT_FILE_BYTES {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn run_git(root: &Path, args: &[&str]) -> Result<Vec<u8>, GitError> {
    run_git_internal(root, args)?.ok_or(GitError::CommandFailed)
}

fn run_git_optional(root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, GitError> {
    run_git_internal(root, args)
}

fn run_git_internal(root: &Path, args: &[&str]) -> Result<Option<Vec<u8>>, GitError> {
    let mut command = Command::new("git");
    command
        .env_clear()
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(root)
        .arg("-c")
        .arg("core.hooksPath=/dev/null")
        .arg("-c")
        .arg("diff.external=")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|_| GitError::CommandFailed)?;
    let mut stdout = child.stdout.take().ok_or(GitError::Io)?;
    let reader = thread::spawn(move || {
        let mut collected = Vec::new();
        let mut oversized = false;
        let mut buffer = [0u8; 8192];
        loop {
            let read = stdout.read(&mut buffer).map_err(|_| GitError::Io)?;
            if read == 0 {
                break;
            }
            if collected.len().saturating_add(read) <= MAX_GIT_COMMAND_BYTES {
                collected.extend_from_slice(&buffer[..read]);
            } else {
                oversized = true;
            }
        }
        if oversized {
            Err(GitError::OutputLimit)
        } else {
            Ok(collected)
        }
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|_| GitError::CommandFailed)? {
            break status;
        }
        if started.elapsed() > GIT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(GitError::Timeout);
        }
        thread::sleep(Duration::from_millis(2));
    };
    let output = reader.join().map_err(|_| GitError::Io)??;
    if status.success() {
        Ok(Some(output))
    } else {
        Ok(None)
    }
}

fn parse_nul_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .filter_map(|value| std::str::from_utf8(value).ok())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_status(output: &[u8]) -> Result<BTreeMap<String, StatusEntry>, GitError> {
    let records = output
        .split(|byte| *byte == 0)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut result = BTreeMap::new();
    let mut index = 0usize;
    while index < records.len() {
        let record = std::str::from_utf8(records[index]).map_err(|_| GitError::InvalidOutput)?;
        if record.len() < 3 {
            return Err(GitError::InvalidOutput);
        }
        let bytes = record.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let path = record[3..].to_owned();
        let mut entry = StatusEntry {
            change: Some(status_change(x, y)),
            staged: x != ' ' && x != '?',
            unstaged: y != ' ' || (x == '?' && y == '?'),
            old_path: None,
        };
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            index += 1;
            let old = records.get(index).ok_or(GitError::InvalidOutput)?;
            entry.old_path = Some(
                std::str::from_utf8(old)
                    .map_err(|_| GitError::InvalidOutput)?
                    .to_owned(),
            );
            entry.change = Some(GitChange::Renamed);
        }
        result.insert(path, entry);
        index += 1;
    }
    Ok(result)
}

fn status_change(x: char, y: char) -> GitChange {
    if x == '?' && y == '?' {
        GitChange::Untracked
    } else if x == 'A' || y == 'A' {
        GitChange::Added
    } else if x == 'D' || y == 'D' {
        GitChange::Deleted
    } else if x == 'R' || y == 'R' || x == 'C' || y == 'C' {
        GitChange::Renamed
    } else if x == 'M' || y == 'M' || x == 'T' || y == 'T' {
        GitChange::Modified
    } else {
        GitChange::Unknown
    }
}

fn hex_sha256(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
