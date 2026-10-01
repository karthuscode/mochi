use crate::{SqliteStore, StorageError, StorageResult, connection::utc_millis, error::map_sqlite};
use mochi_domain::{Project, ProjectId, RepositoryIdentity, UtcTimestamp};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde_json::{from_str, to_string};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapturePolicy {
    pub tracking_enabled: bool,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredProject {
    pub project: Project,
    pub capture_policy: CapturePolicy,
    pub deleted_at: Option<UtcTimestamp>,
}

pub trait ProjectRepository {
    fn create_project(&self, project: &Project, policy: CapturePolicy) -> StorageResult<()>;
    fn get_project(&self, id: ProjectId) -> StorageResult<Option<StoredProject>>;
    fn update_capture_policy(
        &self,
        id: ProjectId,
        tracking_enabled: bool,
    ) -> StorageResult<CapturePolicy>;
    fn soft_delete_project(&self, id: ProjectId) -> StorageResult<bool>;
}

impl ProjectRepository for SqliteStore {
    fn create_project(&self, project: &Project, policy: CapturePolicy) -> StorageResult<()> {
        project
            .validate()
            .map_err(|_| StorageError::DomainValidation)?;
        if policy.revision == 0 {
            return Err(StorageError::InvalidInput);
        }
        let repository_identity = project
            .repository_identity
            .as_ref()
            .map(to_string)
            .transpose()
            .map_err(|_| StorageError::Serialization)?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "INSERT INTO projects(
                    id, display_name, root_path, repository_identity_json, created_at,
                    last_seen_at, tracking_enabled, policy_revision, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
                params![
                    project.id.to_string(),
                    project.display_name,
                    project.root_path,
                    repository_identity,
                    project.created_at.as_str(),
                    project.last_seen_at.as_str(),
                    policy.tracking_enabled,
                    i64::try_from(policy.revision).map_err(|_| StorageError::InvalidInput)?,
                ],
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)
    }

    fn get_project(&self, id: ProjectId) -> StorageResult<Option<StoredProject>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT display_name, root_path, repository_identity_json, created_at,
                        last_seen_at, tracking_enabled, policy_revision, deleted_at
                 FROM projects WHERE id = ?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, bool>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(map_sqlite)?
            .map(|row| decode_project(id, row))
            .transpose()
    }

    fn update_capture_policy(
        &self,
        id: ProjectId,
        tracking_enabled: bool,
    ) -> StorageResult<CapturePolicy> {
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let changed = transaction
            .execute(
                "UPDATE projects
                 SET tracking_enabled = ?2, policy_revision = policy_revision + 1
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), tracking_enabled],
            )
            .map_err(map_sqlite)?;
        if changed == 0 {
            return Err(StorageError::PolicyRejected);
        }
        let revision: i64 = transaction
            .query_row(
                "SELECT policy_revision FROM projects WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(CapturePolicy {
            tracking_enabled,
            revision: u64::try_from(revision).map_err(|_| StorageError::Corrupt)?,
        })
    }

    fn soft_delete_project(&self, id: ProjectId) -> StorageResult<bool> {
        let now = self.now()?;
        let expires_at = tombstone_expiry(&now)?;
        let expires_at_unix_ms = utc_millis(&expires_at)?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let active: bool = transaction
            .query_row(
                "SELECT deleted_at IS NULL FROM projects WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?
            .unwrap_or(false);
        if !active {
            return Ok(false);
        }
        transaction
            .execute(
                "INSERT OR REPLACE INTO ingress_tombstones(
                    identity_key, identity_kind, reason, deleted_at, expires_at, expires_at_unix_ms
                 ) SELECT ingress_id, 'ingress', 'project_deleted', ?2, ?3, ?4
                   FROM ingress_events WHERE project_id = ?1",
                params![id.to_string(), now, expires_at, expires_at_unix_ms],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO ingress_tombstones(
                    identity_key, identity_kind, reason, deleted_at, expires_at, expires_at_unix_ms
                 ) SELECT source_identity_key, 'source', 'project_deleted', ?2, ?3, ?4
                   FROM ingress_events WHERE project_id = ?1",
                params![id.to_string(), now, expires_at, expires_at_unix_ms],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "DELETE FROM ingress_events WHERE project_id = ?1",
                [id.to_string()],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "UPDATE assembly_groups
                 SET state = 'deleted', deleted_at = ?2
                 WHERE project_id = ?1 AND state = 'assembled'",
                params![id.to_string(), now],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "DELETE FROM sessions WHERE project_id = ?1",
                [id.to_string()],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "UPDATE projects
                 SET tracking_enabled = 0, policy_revision = policy_revision + 1, deleted_at = ?2
                 WHERE id = ?1",
                params![id.to_string(), now],
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(true)
    }
}

type ProjectRow = (
    String,
    String,
    Option<String>,
    String,
    String,
    bool,
    i64,
    Option<String>,
);

fn decode_project(id: ProjectId, row: ProjectRow) -> StorageResult<StoredProject> {
    let repository_identity: Option<RepositoryIdentity> = row
        .2
        .as_deref()
        .map(from_str)
        .transpose()
        .map_err(|_| StorageError::Serialization)?;
    let project = Project {
        id,
        display_name: row.0,
        root_path: row.1,
        repository_identity,
        created_at: UtcTimestamp::parse(&row.3).map_err(|_| StorageError::DomainValidation)?,
        last_seen_at: UtcTimestamp::parse(&row.4).map_err(|_| StorageError::DomainValidation)?,
    };
    project
        .validate()
        .map_err(|_| StorageError::DomainValidation)?;
    Ok(StoredProject {
        project,
        capture_policy: CapturePolicy {
            tracking_enabled: row.5,
            revision: u64::try_from(row.6).map_err(|_| StorageError::Corrupt)?,
        },
        deleted_at: row
            .7
            .as_deref()
            .map(UtcTimestamp::parse)
            .transpose()
            .map_err(|_| StorageError::DomainValidation)?,
    })
}

pub(crate) fn tombstone_expiry(now: &str) -> StorageResult<String> {
    let now = OffsetDateTime::parse(now, &Rfc3339).map_err(|_| StorageError::Serialization)?;
    (now + Duration::days(7))
        .format(&Rfc3339)
        .map_err(|_| StorageError::Serialization)
}
