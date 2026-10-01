use crate::{
    PersistedIngress, SqliteStore, StorageError, StorageResult,
    error::map_sqlite,
    ingress::{decode_ingress, decode_ingress_row},
    session::insert_session_transaction,
};
use mochi_capture::model::ClientSurface;
use mochi_domain::{CodingSession, ProjectId, SessionId};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use std::collections::HashSet;
use uuid::Uuid;

pub const DEFAULT_ASSEMBLY_CANDIDATES: usize = 10;
pub const MAX_ASSEMBLY_CANDIDATES: usize = 25;
pub const MAX_ASSEMBLY_EVENTS: usize = 20_000;
pub const MAX_ASSEMBLY_BYTES: usize = 20 * 1024 * 1024;
const MAX_IGNORED_BATCH: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblySourceKey {
    pub project_id: ProjectId,
    pub provider: String,
    pub adapter_version: String,
    pub transport: String,
    pub client_surface: ClientSurface,
    pub external_session_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssemblyCandidate {
    pub source: AssemblySourceKey,
    pub record_count: usize,
    pub serialized_bytes: usize,
    pub first_receive_sequence: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AssemblyEvidenceLoad {
    Ready(Vec<PersistedIngress>),
    BoundsExceeded {
        record_count: usize,
        serialized_bytes: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IgnoredIngressReason {
    MissingSessionIdentity,
    ArrivedAfterAssembly,
    DeletedSession,
}

impl IgnoredIngressReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::MissingSessionIdentity => "missing_session_identity",
            Self::ArrivedAfterAssembly => "arrived_after_assembly",
            Self::DeletedSession => "deleted_session",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssemblyWriteOutcome {
    Inserted,
    Existing(SessionId),
    Deleted(SessionId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IngressAssemblyState {
    Unassigned,
    Assigned { session_id: SessionId },
    Ignored { reason: String },
}

pub struct AssemblyWrite<'a> {
    pub group_key: &'a str,
    pub source: &'a AssemblySourceKey,
    pub session: &'a CodingSession,
    pub ingress_ids: &'a [Uuid],
    pub assembly_version: u16,
    pub evidence_fingerprint: &'a str,
}

pub trait AssemblyRepository {
    fn list_assembly_candidates(
        &self,
        limit: Option<usize>,
    ) -> StorageResult<Vec<AssemblyCandidate>>;
    fn load_assembly_evidence(
        &self,
        source: &AssemblySourceKey,
        max_records: usize,
        max_bytes: usize,
    ) -> StorageResult<AssemblyEvidenceLoad>;
    fn list_unidentified_ingress(&self, limit: usize) -> StorageResult<Vec<PersistedIngress>>;
    fn mark_ingress_ignored(
        &self,
        ingress_ids: &[Uuid],
        reason: IgnoredIngressReason,
        assembly_version: u16,
    ) -> StorageResult<usize>;
    fn persist_assembled_session(
        &self,
        request: AssemblyWrite<'_>,
    ) -> StorageResult<AssemblyWriteOutcome>;
    fn list_session_ingress(&self, session_id: SessionId, limit: usize)
    -> StorageResult<Vec<Uuid>>;
    fn ingress_assembly_state(&self, ingress_id: Uuid) -> StorageResult<IngressAssemblyState>;
}

impl AssemblyRepository for SqliteStore {
    fn list_assembly_candidates(
        &self,
        limit: Option<usize>,
    ) -> StorageResult<Vec<AssemblyCandidate>> {
        let limit = limit.unwrap_or(DEFAULT_ASSEMBLY_CANDIDATES);
        if limit == 0 || limit > MAX_ASSEMBLY_CANDIDATES {
            return Err(StorageError::InvalidInput);
        }
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT e.project_id, e.provider, e.adapter_version, e.transport, e.client_surface,
                        e.external_session_id, COUNT(*),
                        COALESCE(SUM(
                            LENGTH(CAST(e.normalized_event_json AS BLOB))
                            + LENGTH(CAST(e.sensitivity_json AS BLOB))
                            + LENGTH(CAST(e.source_event_type AS BLOB))
                            + LENGTH(CAST(COALESCE(e.source_event_id, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.source_timestamp, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.external_turn_id, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.external_tool_use_id, '') AS BLOB))
                        ), 0),
                        MIN(e.receive_sequence)
                 FROM ingress_events e
                 LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
                 WHERE a.ingress_id IS NULL
                   AND e.external_session_id IS NOT NULL
                   AND LENGTH(TRIM(e.external_session_id)) > 0
                 GROUP BY e.project_id, e.provider, e.adapter_version, e.transport,
                          e.client_surface, e.external_session_id
                 ORDER BY MIN(e.receive_sequence), e.project_id, e.external_session_id
                 LIMIT ?1",
            )
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                [i64::try_from(limit).map_err(|_| StorageError::InvalidInput)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .map_err(map_sqlite)?;
        let mut candidates = Vec::new();
        for row in rows {
            let row = row.map_err(map_sqlite)?;
            candidates.push(AssemblyCandidate {
                source: AssemblySourceKey {
                    project_id: ProjectId::parse(&row.0).map_err(|_| StorageError::Corrupt)?,
                    provider: row.1,
                    adapter_version: row.2,
                    transport: row.3,
                    client_surface: decode_surface(&row.4)?,
                    external_session_id: row.5,
                },
                record_count: usize::try_from(row.6).map_err(|_| StorageError::Corrupt)?,
                serialized_bytes: usize::try_from(row.7).map_err(|_| StorageError::Corrupt)?,
                first_receive_sequence: u64::try_from(row.8).map_err(|_| StorageError::Corrupt)?,
            });
        }
        Ok(candidates)
    }

    fn load_assembly_evidence(
        &self,
        source: &AssemblySourceKey,
        max_records: usize,
        max_bytes: usize,
    ) -> StorageResult<AssemblyEvidenceLoad> {
        if max_records == 0
            || max_records > MAX_ASSEMBLY_EVENTS
            || max_bytes == 0
            || max_bytes > MAX_ASSEMBLY_BYTES
        {
            return Err(StorageError::InvalidInput);
        }
        let connection = self.lock()?;
        let (count, bytes): (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(
                            LENGTH(CAST(e.normalized_event_json AS BLOB))
                            + LENGTH(CAST(e.sensitivity_json AS BLOB))
                            + LENGTH(CAST(e.source_event_type AS BLOB))
                            + LENGTH(CAST(COALESCE(e.source_event_id, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.source_timestamp, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.external_turn_id, '') AS BLOB))
                            + LENGTH(CAST(COALESCE(e.external_tool_use_id, '') AS BLOB))
                        ), 0)
                 FROM ingress_events e
                 LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
                 WHERE a.ingress_id IS NULL AND e.project_id = ?1 AND e.provider = ?2
                   AND e.adapter_version = ?3 AND e.transport = ?4 AND e.client_surface = ?5
                   AND e.external_session_id = ?6",
                params![
                    source.project_id.to_string(),
                    source.provider,
                    source.adapter_version,
                    source.transport,
                    surface_text(source.client_surface),
                    source.external_session_id,
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(map_sqlite)?;
        let count = usize::try_from(count).map_err(|_| StorageError::Corrupt)?;
        let bytes = usize::try_from(bytes).map_err(|_| StorageError::Corrupt)?;
        if count > max_records || bytes > max_bytes {
            return Ok(AssemblyEvidenceLoad::BoundsExceeded {
                record_count: count,
                serialized_bytes: bytes,
            });
        }
        let mut statement = connection
            .prepare(&format!(
                "SELECT e.ingress_id, e.source_identity_key, e.schema_version, e.provider,
                        e.adapter_version, e.transport, e.client_surface, e.source_event_id,
                        e.source_sequence, e.source_timestamp, e.received_at, e.receive_sequence,
                        e.source_event_type, e.external_session_id, e.external_turn_id,
                        e.external_tool_use_id, e.origin, e.normalized_event_json,
                        e.sensitivity_json, e.imported_at
                 FROM ingress_events e
                 LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
                 WHERE a.ingress_id IS NULL AND e.project_id = ?1 AND e.provider = ?2
                   AND e.adapter_version = ?3 AND e.transport = ?4 AND e.client_surface = ?5
                   AND e.external_session_id = ?6
                 ORDER BY e.receive_sequence, e.ingress_id
                 LIMIT {}",
                max_records
            ))
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                params![
                    source.project_id.to_string(),
                    source.provider,
                    source.adapter_version,
                    source.transport,
                    surface_text(source.client_surface),
                    source.external_session_id,
                ],
                decode_ingress_row,
            )
            .map_err(map_sqlite)?;
        let mut evidence = Vec::with_capacity(count);
        for row in rows {
            evidence.push(decode_ingress(source.project_id, row.map_err(map_sqlite)?)?);
        }
        Ok(AssemblyEvidenceLoad::Ready(evidence))
    }

    fn list_unidentified_ingress(&self, limit: usize) -> StorageResult<Vec<PersistedIngress>> {
        if limit == 0 || limit > MAX_IGNORED_BATCH {
            return Err(StorageError::InvalidInput);
        }
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT e.ingress_id, e.source_identity_key, e.schema_version, e.provider,
                        e.adapter_version, e.transport, e.client_surface, e.source_event_id,
                        e.source_sequence, e.source_timestamp, e.received_at, e.receive_sequence,
                        e.source_event_type, e.external_session_id, e.external_turn_id,
                        e.external_tool_use_id, e.origin, e.normalized_event_json,
                        e.sensitivity_json, e.imported_at, e.project_id
                 FROM ingress_events e
                 LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
                 WHERE a.ingress_id IS NULL
                   AND (e.external_session_id IS NULL OR LENGTH(TRIM(e.external_session_id)) = 0)
                 ORDER BY e.receive_sequence, e.ingress_id
                 LIMIT ?1",
            )
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                [i64::try_from(limit).map_err(|_| StorageError::InvalidInput)?],
                |row| {
                    let raw = decode_ingress_row(row)?;
                    Ok((raw, row.get::<_, String>(20)?))
                },
            )
            .map_err(map_sqlite)?;
        let mut evidence = Vec::new();
        for row in rows {
            let (raw, project_id) = row.map_err(map_sqlite)?;
            let project_id = ProjectId::parse(&project_id).map_err(|_| StorageError::Corrupt)?;
            evidence.push(decode_ingress(project_id, raw)?);
        }
        Ok(evidence)
    }

    fn mark_ingress_ignored(
        &self,
        ingress_ids: &[Uuid],
        reason: IgnoredIngressReason,
        assembly_version: u16,
    ) -> StorageResult<usize> {
        if ingress_ids.is_empty() || ingress_ids.len() > MAX_IGNORED_BATCH || assembly_version == 0
        {
            return Err(StorageError::InvalidInput);
        }
        let now = self.now()?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let changed = insert_ignored(
            &transaction,
            ingress_ids,
            reason,
            assembly_version,
            &now,
            None,
        )?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(changed)
    }

    fn persist_assembled_session(
        &self,
        request: AssemblyWrite<'_>,
    ) -> StorageResult<AssemblyWriteOutcome> {
        validate_write(&request)?;
        let now = self.now()?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;

        if let Some((group_key, session_id, state)) = existing_group(&transaction, request.source)?
        {
            if group_key != request.group_key || session_id != request.session.data().id.to_string()
            {
                return Err(StorageError::Constraint);
            }
            let session_id = SessionId::parse(&session_id).map_err(|_| StorageError::Corrupt)?;
            let reason = if state == "deleted" {
                IgnoredIngressReason::DeletedSession
            } else if state == "assembled" {
                IgnoredIngressReason::ArrivedAfterAssembly
            } else {
                return Err(StorageError::Corrupt);
            };
            insert_ignored(
                &transaction,
                request.ingress_ids,
                reason,
                request.assembly_version,
                &now,
                Some(request.source),
            )?;
            transaction.commit().map_err(map_sqlite)?;
            return if state == "deleted" {
                Ok(AssemblyWriteOutcome::Deleted(session_id))
            } else {
                Ok(AssemblyWriteOutcome::Existing(session_id))
            };
        }

        verify_complete_unassigned_group(&transaction, request.source, request.ingress_ids)?;
        insert_session_transaction(&transaction, request.session, &now)?;
        transaction
            .execute(
                "INSERT INTO assembly_groups(
                    group_key, project_id, provider, adapter_version, transport, client_surface,
                    external_session_id, session_id, state, assembly_version,
                    evidence_fingerprint, assembled_at, deleted_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'assembled', ?9, ?10, ?11, NULL)",
                params![
                    request.group_key,
                    request.source.project_id.to_string(),
                    request.source.provider,
                    request.source.adapter_version,
                    request.source.transport,
                    surface_text(request.source.client_surface),
                    request.source.external_session_id,
                    request.session.data().id.to_string(),
                    i64::from(request.assembly_version),
                    request.evidence_fingerprint,
                    now,
                ],
            )
            .map_err(map_sqlite)?;
        for ingress_id in request.ingress_ids {
            transaction
                .execute(
                    "INSERT INTO ingress_assembly_state(
                        ingress_id, group_key, session_id, state, reason, assembly_version, updated_at
                     ) VALUES (?1, ?2, ?3, 'assigned', NULL, ?4, ?5)",
                    params![
                        ingress_id.to_string(),
                        request.group_key,
                        request.session.data().id.to_string(),
                        i64::from(request.assembly_version),
                        now,
                    ],
                )
                .map_err(map_sqlite)?;
        }
        transaction.commit().map_err(map_sqlite)?;
        Ok(AssemblyWriteOutcome::Inserted)
    }

    fn list_session_ingress(
        &self,
        session_id: SessionId,
        limit: usize,
    ) -> StorageResult<Vec<Uuid>> {
        if limit == 0 || limit > MAX_ASSEMBLY_EVENTS {
            return Err(StorageError::InvalidInput);
        }
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT a.ingress_id FROM ingress_assembly_state a
                 JOIN ingress_events e ON e.ingress_id = a.ingress_id
                 WHERE a.session_id = ?1 AND a.state = 'assigned'
                 ORDER BY e.receive_sequence, a.ingress_id LIMIT ?2",
            )
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                params![
                    session_id.to_string(),
                    i64::try_from(limit).map_err(|_| StorageError::InvalidInput)?
                ],
                |row| row.get::<_, String>(0),
            )
            .map_err(map_sqlite)?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(
                Uuid::parse_str(&row.map_err(map_sqlite)?).map_err(|_| StorageError::Corrupt)?,
            );
        }
        Ok(ids)
    }

    fn ingress_assembly_state(&self, ingress_id: Uuid) -> StorageResult<IngressAssemblyState> {
        let connection = self.lock()?;
        let row: Option<(String, Option<String>, Option<String>)> = connection
            .query_row(
                "SELECT state, session_id, reason FROM ingress_assembly_state WHERE ingress_id = ?1",
                [ingress_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_sqlite)?;
        match row {
            None => Ok(IngressAssemblyState::Unassigned),
            Some((state, Some(session_id), None)) if state == "assigned" => {
                Ok(IngressAssemblyState::Assigned {
                    session_id: SessionId::parse(&session_id).map_err(|_| StorageError::Corrupt)?,
                })
            }
            Some((state, None, Some(reason))) if state == "ignored" => {
                Ok(IngressAssemblyState::Ignored { reason })
            }
            _ => Err(StorageError::Corrupt),
        }
    }
}

fn validate_write(request: &AssemblyWrite<'_>) -> StorageResult<()> {
    if request.group_key.is_empty()
        || request.group_key.len() > 256
        || request.evidence_fingerprint.is_empty()
        || request.evidence_fingerprint.len() > 128
        || request.assembly_version == 0
        || request.ingress_ids.is_empty()
        || request.ingress_ids.len() > MAX_ASSEMBLY_EVENTS
        || request.session.data().project_id != request.source.project_id
        || request.session.data().source.provider != request.source.provider
        || request.session.data().source.adapter_version != request.source.adapter_version
        || request.session.data().source.transport != request.source.transport
        || domain_surface(request.session.data().source.client_surface)
            != request.source.client_surface
    {
        return Err(StorageError::InvalidInput);
    }
    let unique = request.ingress_ids.iter().copied().collect::<HashSet<_>>();
    if unique.len() != request.ingress_ids.len() {
        return Err(StorageError::InvalidInput);
    }
    Ok(())
}

fn existing_group(
    transaction: &Transaction<'_>,
    source: &AssemblySourceKey,
) -> StorageResult<Option<(String, String, String)>> {
    transaction
        .query_row(
            "SELECT group_key, session_id, state FROM assembly_groups
             WHERE project_id = ?1 AND provider = ?2 AND adapter_version = ?3
               AND transport = ?4 AND client_surface = ?5 AND external_session_id = ?6",
            params![
                source.project_id.to_string(),
                source.provider,
                source.adapter_version,
                source.transport,
                surface_text(source.client_surface),
                source.external_session_id,
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(map_sqlite)
}

fn verify_complete_unassigned_group(
    transaction: &Transaction<'_>,
    source: &AssemblySourceKey,
    ingress_ids: &[Uuid],
) -> StorageResult<()> {
    let count: i64 = transaction
        .query_row(
            "SELECT COUNT(*) FROM ingress_events e
             LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
             WHERE a.ingress_id IS NULL AND e.project_id = ?1 AND e.provider = ?2
               AND e.adapter_version = ?3 AND e.transport = ?4 AND e.client_surface = ?5
               AND e.external_session_id = ?6",
            params![
                source.project_id.to_string(),
                source.provider,
                source.adapter_version,
                source.transport,
                surface_text(source.client_surface),
                source.external_session_id,
            ],
            |row| row.get(0),
        )
        .map_err(map_sqlite)?;
    if usize::try_from(count).map_err(|_| StorageError::Corrupt)? != ingress_ids.len() {
        return Err(StorageError::Constraint);
    }
    for ingress_id in ingress_ids {
        let matches: bool = transaction
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM ingress_events e
                    LEFT JOIN ingress_assembly_state a ON a.ingress_id = e.ingress_id
                    WHERE e.ingress_id = ?1 AND a.ingress_id IS NULL
                      AND e.project_id = ?2 AND e.provider = ?3 AND e.adapter_version = ?4
                      AND e.transport = ?5 AND e.client_surface = ?6
                      AND e.external_session_id = ?7
                 )",
                params![
                    ingress_id.to_string(),
                    source.project_id.to_string(),
                    source.provider,
                    source.adapter_version,
                    source.transport,
                    surface_text(source.client_surface),
                    source.external_session_id,
                ],
                |row| row.get(0),
            )
            .map_err(map_sqlite)?;
        if !matches {
            return Err(StorageError::Constraint);
        }
    }
    Ok(())
}

fn insert_ignored(
    transaction: &Transaction<'_>,
    ingress_ids: &[Uuid],
    reason: IgnoredIngressReason,
    assembly_version: u16,
    now: &str,
    expected_source: Option<&AssemblySourceKey>,
) -> StorageResult<usize> {
    let mut changed = 0;
    for ingress_id in ingress_ids {
        let exists: bool = if let Some(source) = expected_source {
            transaction
                .query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM ingress_events
                        WHERE ingress_id = ?1 AND project_id = ?2 AND provider = ?3
                          AND adapter_version = ?4 AND transport = ?5 AND client_surface = ?6
                          AND external_session_id = ?7
                     )",
                    params![
                        ingress_id.to_string(),
                        source.project_id.to_string(),
                        source.provider,
                        source.adapter_version,
                        source.transport,
                        surface_text(source.client_surface),
                        source.external_session_id,
                    ],
                    |row| row.get(0),
                )
                .map_err(map_sqlite)?
        } else {
            transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM ingress_events WHERE ingress_id = ?1)",
                    [ingress_id.to_string()],
                    |row| row.get(0),
                )
                .map_err(map_sqlite)?
        };
        if !exists {
            return Err(StorageError::Constraint);
        }
        changed += transaction
            .execute(
                "INSERT OR IGNORE INTO ingress_assembly_state(
                    ingress_id, group_key, session_id, state, reason, assembly_version, updated_at
                 ) VALUES (?1, NULL, NULL, 'ignored', ?2, ?3, ?4)",
                params![
                    ingress_id.to_string(),
                    reason.as_str(),
                    i64::from(assembly_version),
                    now,
                ],
            )
            .map_err(map_sqlite)?;
    }
    Ok(changed)
}

fn decode_surface(value: &str) -> StorageResult<ClientSurface> {
    match value {
        "cli" => Ok(ClientSurface::Cli),
        "desktop" => Ok(ClientSurface::Desktop),
        "unknown" => Ok(ClientSurface::Unknown),
        _ => Err(StorageError::Corrupt),
    }
}

fn surface_text(value: ClientSurface) -> &'static str {
    match value {
        ClientSurface::Cli => "cli",
        ClientSurface::Desktop => "desktop",
        ClientSurface::Unknown => "unknown",
    }
}

fn domain_surface(value: mochi_domain::ClientSurface) -> ClientSurface {
    match value {
        mochi_domain::ClientSurface::Cli => ClientSurface::Cli,
        mochi_domain::ClientSurface::Desktop => ClientSurface::Desktop,
        mochi_domain::ClientSurface::Unknown => ClientSurface::Unknown,
    }
}
