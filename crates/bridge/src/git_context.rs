use crate::{BridgeError, CaptureBridge};
use mochi_capture::{
    git::{GitChange, GitContentOmission as Omission, GitError, GitSnapshot},
    GitContextReader, SnapshotRole,
};
use mochi_domain::{
    FileChangeKind, GitContext, GitFileState, GitSnapshot as DomainSnapshot, GitUnavailableReason,
    SessionId, UtcTimestamp, WorkingTreeState,
};
use mochi_persistence::{CodingSessionRepository, EpisodeState, ProjectRepository, SqliteStore};

impl CaptureBridge {
    /// Holds current consent through filtered reading and the caller's durable
    /// publication. This can never run inside a Codex hook.
    pub fn collect_session_git(
        &self,
        store: &SqliteStore,
        id: SessionId,
        reader: &impl GitContextReader,
        publish: impl FnOnce(&GitContext) -> Result<(), BridgeError>,
    ) -> Result<bool, BridgeError> {
        let ep = store
            .get_episode(id)
            .map_err(|_| BridgeError::Storage)?
            .ok_or(BridgeError::UnknownProject)?;
        if ep.state == EpisodeState::Deleted {
            return Ok(false);
        }
        let session = store
            .get_session(id)
            .map_err(|_| BridgeError::Storage)?
            .ok_or(BridgeError::Storage)?;
        let current = &session.data().git_context;
        let finished = ep.state == EpisodeState::Finalized;
        let need = matches!(
            current,
            GitContext::Unavailable {
                reason: GitUnavailableReason::NotCaptured
            }
        ) || (finished && matches!(current, GitContext::Available { after: None, .. }));
        if !need {
            return Ok(false);
        }
        let project = store
            .get_project(ep.source.project_id)
            .map_err(|_| BridgeError::Storage)?
            .filter(|p| p.deleted_at.is_none() && p.capture_policy.tracking_enabled)
            .ok_or(BridgeError::Authorization)?;
        let lease = self
            .policies
            .authorize(ep.source.project_id.as_uuid())
            .map_err(|_| BridgeError::Authorization)?;
        let policy = lease.policy();
        if policy.policy_revision != project.capture_policy.revision
            || policy.approved_root != std::path::Path::new(&project.project.root_path)
            || policy.spool_root != self.spool_root
        {
            return Err(BridgeError::Authorization);
        }
        let role = if finished {
            SnapshotRole::Final
        } else {
            SnapshotRole::Baseline
        };
        let context =
            match reader.snapshot(ep.source.project_id.as_uuid(), &policy.approved_root, role) {
                Ok(snapshot) => {
                    let snapshot = convert(snapshot)?;
                    if finished {
                        if let GitContext::Available { before, .. } = current {
                            GitContext::Available {
                                before: before.clone(),
                                after: Some(Box::new(snapshot)),
                            }
                        } else {
                            GitContext::FinalOnly {
                                after: Box::new(snapshot),
                                baseline_reason: GitUnavailableReason::NotCaptured,
                            }
                        }
                    } else {
                        GitContext::Available {
                            before: Box::new(snapshot),
                            after: None,
                        }
                    }
                }
                Err(GitError::NotRepository) => GitContext::Unavailable {
                    reason: GitUnavailableReason::NotRepository,
                },
                Err(_) => GitContext::Unavailable {
                    reason: GitUnavailableReason::CollectionFailed,
                },
            };
        context.validate().map_err(|_| BridgeError::Storage)?;
        publish(&context)?;
        drop(lease);
        Ok(true)
    }
}
fn convert(s: GitSnapshot) -> Result<DomainSnapshot, BridgeError> {
    let dirty = s.files.iter().any(|f| f.change != GitChange::Clean);
    let mut warnings =
        vec!["Snapshot collection is asynchronous; changes have ambiguous attribution.".into()];
    if s.role == SnapshotRole::Baseline {
        warnings.push(
            "Baseline captured after observed activity; earlier changes may be missing.".into(),
        );
    }
    Ok(DomainSnapshot {
        repository_root: s.repository_root,
        branch: s.branch,
        detached: Some(s.detached),
        head_commit: s.head,
        captured_at: UtcTimestamp::parse(&s.captured_at).map_err(|_| BridgeError::Storage)?,
        working_tree_state: if dirty {
            WorkingTreeState::Dirty
        } else {
            WorkingTreeState::Clean
        },
        files: s
            .files
            .into_iter()
            .map(|f| GitFileState {
                path: f.path,
                previous_path: f.old_path,
                change_type: match f.change {
                    GitChange::Added | GitChange::Untracked => FileChangeKind::Added,
                    GitChange::Modified => FileChangeKind::Modified,
                    GitChange::Deleted => FileChangeKind::Deleted,
                    GitChange::Renamed => FileChangeKind::Renamed,
                    _ => FileChangeKind::Unknown,
                },
                staged: Some(f.staged),
                unstaged: Some(f.unstaged),
                content_hash: f.content_hash,
                content: f.content,
                byte_count: f.size_bytes,
                omission: f.omission.map(|o| match o {
                    Omission::Deleted => mochi_domain::GitContentOmission::Deleted,
                    Omission::Binary => mochi_domain::GitContentOmission::Binary,
                    Omission::SizeLimit => mochi_domain::GitContentOmission::SizeLimit,
                    Omission::AggregateLimit | Omission::Policy => {
                        mochi_domain::GitContentOmission::AggregateLimit
                    }
                    Omission::Symlink => mochi_domain::GitContentOmission::Symlink,
                    Omission::Unreadable => mochi_domain::GitContentOmission::Unreadable,
                }),
                truncated: f.omission.is_some(),
            })
            .collect(),
        diff_stats: None,
        excluded_count: u32::try_from(s.excluded_count).map_err(|_| BridgeError::Storage)?,
        omitted_file_count: u32::try_from(s.omitted_file_count)
            .map_err(|_| BridgeError::Storage)?,
        truncated: s.truncated,
        warnings,
    })
}
