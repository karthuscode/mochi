use crate::{DomainError, DomainResult, FileChangeKind, UtcTimestamp};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkingTreeState {
    Clean,
    Dirty,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitFileState {
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: FileChangeKind,
    pub staged: Option<bool>,
    pub unstaged: Option<bool>,
    pub content_hash: Option<String>,
    pub content: Option<String>,
    pub byte_count: Option<u64>,
    pub omission: Option<GitContentOmission>,
    pub truncated: bool,
}

impl GitFileState {
    pub(crate) fn validate(&self) -> DomainResult<()> {
        validate_relative_path(&self.path)?;
        if let Some(path) = &self.previous_path {
            validate_relative_path(path)?;
        }
        if self.change_type == FileChangeKind::Renamed && self.previous_path.is_none() {
            return Err(DomainError::InvalidValue {
                field: "gitFileState.previousPath",
                reason: "renamed files require a previous path",
            });
        }
        Ok(())
    }
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitDiffStats {
    pub files_changed: u32,
    pub insertions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitSnapshot {
    pub repository_root: String,
    pub branch: Option<String>,
    pub detached: Option<bool>,
    pub head_commit: Option<String>,
    pub captured_at: UtcTimestamp,
    pub working_tree_state: WorkingTreeState,
    pub files: Vec<GitFileState>,
    pub diff_stats: Option<GitDiffStats>,
    pub excluded_count: u32,
    pub omitted_file_count: u32,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

impl GitSnapshot {
    pub fn validate(&self) -> DomainResult<()> {
        if self.repository_root.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "gitSnapshot.repositoryRoot",
                reason: "must not be empty",
            });
        }
        for file in &self.files {
            file.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GitUnavailableReason {
    NotRepository,
    NotAuthorized,
    CollectionFailed,
    NotCaptured,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum GitContext {
    FinalOnly {
        after: Box<GitSnapshot>,
        #[serde(rename = "baselineReason")]
        baseline_reason: GitUnavailableReason,
    },
    Available {
        before: Box<GitSnapshot>,
        after: Option<Box<GitSnapshot>>,
    },
    Unavailable {
        reason: GitUnavailableReason,
    },
}

impl GitContext {
    pub fn validate(&self) -> DomainResult<()> {
        if let Self::FinalOnly { after, .. } = self {
            after.validate()?;
        }
        if let Self::Available { before, after } = self {
            before.validate()?;
            if let Some(after) = after {
                after.validate()?;
                if before.repository_root != after.repository_root {
                    return Err(DomainError::InvalidReference {
                        entity: "git context",
                        reason: "before and after snapshots must use the same repository root",
                    });
                }
                if before.captured_at > after.captured_at {
                    return Err(DomainError::InvalidTimeRange {
                        entity: "git context",
                    });
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_relative_path(value: &str) -> DomainResult<()> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(DomainError::InvalidValue {
            field: "relativePath",
            reason: "must remain inside the approved root",
        });
    }
    Ok(())
}
