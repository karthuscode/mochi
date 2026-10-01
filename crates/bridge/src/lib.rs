//! Consent coordination and bounded handoff, independent of UI/provider input.
mod git_context;
use mochi_capture::{
    authorization::{AuthorizationStore, CaptureAuthorization},
    Spool, SpoolLimits,
};
use mochi_domain::ProjectId;
use mochi_persistence::{
    CapturePolicy, ImportOutcome, IngressRepository, ProjectRepository, SqliteStore,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeError {
    Authorization,
    Storage,
    UnknownProject,
}
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "capture: {self:?}")
    }
}
impl std::error::Error for BridgeError {}
type Result<T> = std::result::Result<T, BridgeError>;

pub struct CaptureBridge {
    policies: AuthorizationStore,
    spool_root: PathBuf,
}
impl CaptureBridge {
    pub fn new(policy_root: PathBuf, spool_root: PathBuf) -> Result<Self> {
        if !spool_root.is_absolute() {
            return Err(BridgeError::Authorization);
        }
        Ok(Self {
            policies: AuthorizationStore::create(policy_root)
                .map_err(|_| BridgeError::Authorization)?,
            spool_root,
        })
    }
    /// Called only after explicit local tracking approval. Disabling the policy
    /// before the DB transition makes crash/failure windows fail closed.
    pub fn set_tracking(
        &self,
        store: &SqliteStore,
        id: ProjectId,
        enabled: bool,
    ) -> Result<CapturePolicy> {
        let project = store
            .get_project(id)
            .map_err(|_| BridgeError::Storage)?
            .filter(|p| p.deleted_at.is_none())
            .ok_or(BridgeError::UnknownProject)?;
        let writer = self
            .policies
            .writer(id.as_uuid())
            .map_err(|_| BridgeError::Authorization)?;
        let mut policy = CaptureAuthorization {
            schema_version: 1,
            project_id: id.as_uuid(),
            approved_root: PathBuf::from(&project.project.root_path),
            spool_root: self.spool_root.clone(),
            policy_revision: project.capture_policy.revision,
            global_enabled: false,
            tracking_enabled: false,
        };
        writer
            .publish(&policy)
            .map_err(|_| BridgeError::Authorization)?;
        let db_policy = store
            .update_capture_policy(id, enabled)
            .map_err(|_| BridgeError::Storage)?;
        policy.policy_revision = db_policy.revision;
        policy.global_enabled = enabled;
        policy.tracking_enabled = enabled;
        writer
            .publish(&policy)
            .map_err(|_| BridgeError::Authorization)?;
        Ok(db_policy)
    }
    /// Reconciles a known persisted consent; this never creates a project or
    /// grants consent. The caller loads only bounded existing project rows.
    pub fn reconcile(&self, store: &SqliteStore, id: ProjectId) -> Result<()> {
        let project = store
            .get_project(id)
            .map_err(|_| BridgeError::Storage)?
            .ok_or(BridgeError::UnknownProject)?;
        let writer = self
            .policies
            .writer(id.as_uuid())
            .map_err(|_| BridgeError::Authorization)?;
        let enabled = project.deleted_at.is_none()
            && project.capture_policy.tracking_enabled
            && writer.current_policy().is_ok_and(|p| {
                p.global_enabled
                    && p.tracking_enabled
                    && p.policy_revision == project.capture_policy.revision
                    && p.approved_root == std::path::Path::new(&project.project.root_path)
                    && p.spool_root == self.spool_root
            });
        let revision = if !enabled
            && project.deleted_at.is_none()
            && project.capture_policy.tracking_enabled
        {
            store
                .update_capture_policy(id, false)
                .map_err(|_| BridgeError::Storage)?
                .revision
        } else {
            project.capture_policy.revision
        };
        writer
            .publish(&CaptureAuthorization {
                schema_version: 1,
                project_id: id.as_uuid(),
                approved_root: PathBuf::from(project.project.root_path),
                spool_root: self.spool_root.clone(),
                policy_revision: revision,
                global_enabled: enabled,
                tracking_enabled: enabled,
            })
            .map_err(|_| BridgeError::Authorization)
    }
    pub fn delete_project(&self, store: &SqliteStore, id: ProjectId) -> Result<bool> {
        self.set_tracking(store, id, false)?;
        store
            .soft_delete_project(id)
            .map_err(|_| BridgeError::Storage)
    }
    pub fn import(&self, store: &SqliteStore) -> Result<ImportOutcome> {
        store
            .import_spool_batch(
                &Spool::new(self.spool_root.clone(), SpoolLimits::default()),
                100,
            )
            .map_err(|_| BridgeError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mochi_domain::{Project, UtcTimestamp};
    #[test]
    fn consent_transition_rechecks_db_and_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("project")).unwrap();
        let store = SqliteStore::open_system(root.join("data/mochi.sqlite3")).unwrap();
        let now = UtcTimestamp::parse("2026-10-01T12:00:00Z").unwrap();
        let id = ProjectId::new();
        store
            .create_project(
                &Project {
                    id,
                    display_name: "Synthetic".into(),
                    root_path: root.join("project").to_string_lossy().into(),
                    repository_identity: None,
                    created_at: now.clone(),
                    last_seen_at: now,
                },
                CapturePolicy {
                    tracking_enabled: false,
                    revision: 1,
                },
            )
            .unwrap();
        let bridge = CaptureBridge::new(root.join("policies"), root.join("spool")).unwrap();
        bridge.reconcile(&store, id).unwrap();
        assert!(bridge.policies.authorize(id.as_uuid()).is_err());
        assert_eq!(bridge.set_tracking(&store, id, true).unwrap().revision, 2);
        assert_eq!(
            bridge
                .policies
                .authorize(id.as_uuid())
                .unwrap()
                .policy()
                .policy_revision,
            2
        );
        bridge.reconcile(&store, id).unwrap();
        assert!(bridge.policies.authorize(id.as_uuid()).is_ok());
        // Simulate a crash after revocation reached disk but before the DB update.
        {
            let writer = bridge.policies.writer(id.as_uuid()).unwrap();
            let mut policy = writer.current_policy().unwrap();
            policy.global_enabled = false;
            writer.publish(&policy).unwrap();
        }
        bridge.reconcile(&store, id).unwrap();
        assert!(
            !store
                .get_project(id)
                .unwrap()
                .unwrap()
                .capture_policy
                .tracking_enabled
        );
        assert!(bridge.policies.authorize(id.as_uuid()).is_err());
        assert_eq!(bridge.set_tracking(&store, id, false).unwrap().revision, 4);
        bridge.reconcile(&store, id).unwrap();
        assert!(bridge.policies.authorize(id.as_uuid()).is_err());
        assert!(bridge.set_tracking(&store, ProjectId::new(), true).is_err());
        assert!(bridge.delete_project(&store, id).unwrap());
        assert!(bridge.policies.authorize(id.as_uuid()).is_err());
    }
}
