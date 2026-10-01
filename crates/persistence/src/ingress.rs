use crate::{
    SqliteStore, StorageError, StorageResult, connection::utc_millis, error::map_sqlite,
    project::tombstone_expiry,
};
use mochi_capture::{
    Spool, SpoolIngressRecord,
    model::{
        ClientSurface, INGRESS_SCHEMA_VERSION, NormalizedEvent, Sensitivity, SourceDescriptor,
    },
    spool::{SpoolCandidateData, SpoolRejectionReason},
};
use mochi_domain::ProjectId;
use mochi_privacy::RedactionEngine;
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 100;
const MAX_TEXT_FIELD: usize = 4 * 1024;
const MAX_EVENT_JSON: usize = 64 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ImportOutcome {
    pub inserted: usize,
    pub duplicate: usize,
    pub tombstoned: usize,
    pub rejected: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IngressCursor {
    pub receive_sequence: u64,
    pub ingress_id: Uuid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PersistedIngress {
    pub source_identity_key: String,
    pub record: SpoolIngressRecord,
    pub imported_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IngressPage {
    pub items: Vec<PersistedIngress>,
    pub next_cursor: Option<IngressCursor>,
}

pub trait IngressRepository {
    fn import_spool_batch(&self, spool: &Spool, limit: usize) -> StorageResult<ImportOutcome>;
    fn page_ingress(
        &self,
        project_id: ProjectId,
        after: Option<&IngressCursor>,
        limit: Option<usize>,
    ) -> StorageResult<IngressPage>;
    fn delete_ingress(&self, ingress_id: Uuid) -> StorageResult<bool>;
    fn purge_ingress_before(&self, cutoff: &str) -> StorageResult<usize>;
}

impl IngressRepository for SqliteStore {
    fn import_spool_batch(&self, spool: &Spool, limit: usize) -> StorageResult<ImportOutcome> {
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(StorageError::InvalidInput);
        }
        let candidates = spool
            .scan_batch(limit)
            .map_err(|_| StorageError::InvalidInput)?;
        let spool_state = spool.read_state().map_err(|_| StorageError::InvalidInput)?;
        if let Some(timestamp) = &spool_state.last_eviction_at {
            utc_millis(timestamp)?;
        }
        if candidates.is_empty() && self.spool_eviction_count()? >= spool_state.evicted_count {
            return Ok(ImportOutcome::default());
        }
        let redactor = RedactionEngine::new().map_err(|_| StorageError::Serialization)?;
        let now = self.now()?;
        let now_unix_ms = utc_millis(&now)?;
        let expires_at = tombstone_expiry(&now)?;
        let mut tokens = Vec::with_capacity(candidates.len());
        let mut outcome = ImportOutcome::default();
        {
            let mut connection = self.lock()?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(map_sqlite)?;
            transaction
                .execute(
                    "DELETE FROM ingress_tombstones WHERE expires_at_unix_ms <= ?1",
                    [now_unix_ms],
                )
                .map_err(map_sqlite)?;
            record_spool_state(&transaction, &spool_state, &now)?;

            for candidate in candidates {
                let (token, data) = candidate.into_parts();
                let token_hash = token.opaque_id();
                match data {
                    SpoolCandidateData::Rejected {
                        reason,
                        receive_sequence,
                    } => {
                        insert_rejection(
                            &transaction,
                            &token_hash,
                            rejection_name(reason),
                            receive_sequence,
                            &now,
                        )?;
                        outcome.rejected += 1;
                    }
                    SpoolCandidateData::Record(record) => {
                        if !privacy_safe_record(&redactor, &record) {
                            insert_rejection(
                                &transaction,
                                &token_hash,
                                "redaction_rejected",
                                i64::try_from(record.receive_sequence)
                                    .ok()
                                    .map(|_| record.receive_sequence),
                                &now,
                            )?;
                            insert_tombstone(
                                &transaction,
                                &record.id.to_string(),
                                "ingress",
                                "redaction_rejected",
                                &now,
                                &expires_at,
                            )?;
                            outcome.rejected += 1;
                        } else if validate_record(&record).is_err() {
                            insert_rejection(
                                &transaction,
                                &token_hash,
                                "invalid_record",
                                Some(record.receive_sequence),
                                &now,
                            )?;
                            tombstone_record(
                                &transaction,
                                &record,
                                "invalid_record",
                                &now,
                                &expires_at,
                            )?;
                            outcome.rejected += 1;
                        } else {
                            let source_key = source_identity_key(&record);
                            if is_tombstoned(&transaction, &record.id.to_string(), &source_key)? {
                                outcome.tombstoned += 1;
                            } else if is_duplicate(
                                &transaction,
                                &record.id.to_string(),
                                &source_key,
                            )? {
                                outcome.duplicate += 1;
                            } else if !policy_allows(&transaction, &record)? {
                                insert_rejection(
                                    &transaction,
                                    &token_hash,
                                    "policy_rejected",
                                    Some(record.receive_sequence),
                                    &now,
                                )?;
                                tombstone_record(
                                    &transaction,
                                    &record,
                                    "policy_rejected",
                                    &now,
                                    &expires_at,
                                )?;
                                outcome.rejected += 1;
                            } else {
                                insert_record(&transaction, &record, &source_key, &now)?;
                                outcome.inserted += 1;
                            }
                        }
                    }
                }
                tokens.push(token);
            }
            transaction.commit().map_err(map_sqlite)?;
        }
        spool
            .acknowledge(&tokens)
            .map_err(|_| StorageError::Acknowledgement)?;
        Ok(outcome)
    }

    fn page_ingress(
        &self,
        project_id: ProjectId,
        after: Option<&IngressCursor>,
        limit: Option<usize>,
    ) -> StorageResult<IngressPage> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(StorageError::InvalidInput);
        }
        let after_sequence = after.map(|cursor| cursor.receive_sequence).unwrap_or(0);
        let after_id = after
            .map(|cursor| cursor.ingress_id.to_string())
            .unwrap_or_default();
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT ingress_id, source_identity_key, schema_version, provider,
                        adapter_version, transport, client_surface, source_event_id,
                        source_sequence, source_timestamp, received_at, receive_sequence,
                        source_event_type, external_session_id, external_turn_id,
                        external_tool_use_id, origin, normalized_event_json,
                        sensitivity_json, imported_at
                 FROM ingress_events
                 WHERE project_id = ?1
                   AND (receive_sequence > ?2 OR (receive_sequence = ?2 AND ingress_id > ?3))
                 ORDER BY receive_sequence, ingress_id
                 LIMIT ?4",
            )
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                params![
                    project_id.to_string(),
                    i64::try_from(after_sequence).map_err(|_| StorageError::InvalidInput)?,
                    after_id,
                    i64::try_from(limit).map_err(|_| StorageError::InvalidInput)?,
                ],
                decode_ingress_row,
            )
            .map_err(map_sqlite)?;
        let mut items = Vec::new();
        for row in rows {
            let raw = row.map_err(map_sqlite)?;
            items.push(decode_ingress(project_id, raw)?);
        }
        let next_cursor = items.last().map(|item| IngressCursor {
            receive_sequence: item.record.receive_sequence,
            ingress_id: item.record.id,
        });
        Ok(IngressPage { items, next_cursor })
    }

    fn delete_ingress(&self, ingress_id: Uuid) -> StorageResult<bool> {
        let now = self.now()?;
        let expires_at = tombstone_expiry(&now)?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let source_key: Option<String> = transaction
            .query_row(
                "SELECT source_identity_key FROM ingress_events WHERE ingress_id = ?1",
                [ingress_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        let Some(source_key) = source_key else {
            return Ok(false);
        };
        insert_tombstone(
            &transaction,
            &ingress_id.to_string(),
            "ingress",
            "deleted",
            &now,
            &expires_at,
        )?;
        insert_tombstone(
            &transaction,
            &source_key,
            "source",
            "deleted",
            &now,
            &expires_at,
        )?;
        transaction
            .execute(
                "DELETE FROM ingress_events WHERE ingress_id = ?1",
                [ingress_id.to_string()],
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(true)
    }

    fn purge_ingress_before(&self, cutoff: &str) -> StorageResult<usize> {
        parse_utc(cutoff)?;
        let cutoff_unix_ms = utc_millis(cutoff)?;
        let now = self.now()?;
        let expires_at = tombstone_expiry(&now)?;
        let expires_at_unix_ms = utc_millis(&expires_at)?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO ingress_tombstones(
                identity_key, identity_kind, reason, deleted_at, expires_at, expires_at_unix_ms
             ) SELECT ingress_id, 'ingress', 'retention', ?2, ?3, ?4
               FROM ingress_events WHERE received_at_unix_ms < ?1",
                params![cutoff_unix_ms, now, expires_at, expires_at_unix_ms],
            )
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO ingress_tombstones(
                identity_key, identity_kind, reason, deleted_at, expires_at, expires_at_unix_ms
             ) SELECT source_identity_key, 'source', 'retention', ?2, ?3, ?4
               FROM ingress_events WHERE received_at_unix_ms < ?1",
                params![cutoff_unix_ms, now, expires_at, expires_at_unix_ms],
            )
            .map_err(map_sqlite)?;
        let deleted = transaction
            .execute(
                "DELETE FROM ingress_events WHERE received_at_unix_ms < ?1",
                [cutoff_unix_ms],
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(deleted)
    }
}

fn privacy_safe_record(redactor: &RedactionEngine, record: &SpoolIngressRecord) -> bool {
    serde_json::to_value(record)
        .ok()
        .and_then(|value| redactor.redact_value(&value).ok())
        .is_some_and(|result| !result.changed)
}

fn validate_record(record: &SpoolIngressRecord) -> StorageResult<()> {
    if record.schema_version != INGRESS_SCHEMA_VERSION || record.receive_sequence == 0 {
        return Err(StorageError::InvalidInput);
    }
    parse_utc(&record.received_at)?;
    let fields = [
        record.source.provider.as_str(),
        record.source.adapter_version.as_str(),
        record.source.transport.as_str(),
        record.source_event_type.as_str(),
    ];
    if fields
        .iter()
        .any(|value| value.is_empty() || value.len() > MAX_TEXT_FIELD)
    {
        return Err(StorageError::InvalidInput);
    }
    for value in [
        record.source_event_id.as_deref(),
        record.source_timestamp.as_deref(),
        record.external_session_id.as_deref(),
        record.external_turn_id.as_deref(),
        record.external_tool_use_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if value.len() > MAX_TEXT_FIELD {
            return Err(StorageError::InvalidInput);
        }
    }
    if record
        .source_sequence
        .is_some_and(|value| value > i64::MAX as u64)
        || record.receive_sequence > i64::MAX as u64
        || record.sensitivity.policy_revision == 0
        || record.sensitivity.policy_revision > i64::MAX as u64
        || serde_json::to_vec(&record.event)
            .map_err(|_| StorageError::Serialization)?
            .len()
            > MAX_EVENT_JSON
    {
        return Err(StorageError::InvalidInput);
    }
    Ok(())
}

fn policy_allows(
    transaction: &Transaction<'_>,
    record: &SpoolIngressRecord,
) -> StorageResult<bool> {
    transaction
        .query_row(
            "SELECT tracking_enabled = 1 AND deleted_at IS NULL AND policy_revision = ?2
             FROM projects WHERE id = ?1",
            params![
                record.project_id.to_string(),
                i64::try_from(record.sensitivity.policy_revision)
                    .map_err(|_| StorageError::InvalidInput)?,
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)
        .map(|value| value.unwrap_or(false))
}

fn is_tombstoned(
    transaction: &Transaction<'_>,
    ingress_id: &str,
    source_key: &str,
) -> StorageResult<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ingress_tombstones WHERE identity_key IN (?1, ?2))",
            params![ingress_id, source_key],
            |row| row.get(0),
        )
        .map_err(map_sqlite)
}

fn is_duplicate(
    transaction: &Transaction<'_>,
    ingress_id: &str,
    source_key: &str,
) -> StorageResult<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM ingress_events WHERE ingress_id = ?1 OR source_identity_key = ?2)",
            params![ingress_id, source_key],
            |row| row.get(0),
        )
        .map_err(map_sqlite)
}

fn insert_record(
    transaction: &Transaction<'_>,
    record: &SpoolIngressRecord,
    source_key: &str,
    now: &str,
) -> StorageResult<()> {
    transaction
        .execute(
            "INSERT INTO ingress_events(
            ingress_id, source_identity_key, project_id, schema_version, provider,
            adapter_version, transport, client_surface, source_event_id, source_sequence,
            source_timestamp, received_at, received_at_unix_ms, receive_sequence, source_event_type,
            external_session_id, external_turn_id, external_tool_use_id, origin,
            normalized_event_json, sensitivity_json, imported_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                record.id.to_string(),
                source_key,
                record.project_id.to_string(),
                record.schema_version,
                record.source.provider,
                record.source.adapter_version,
                record.source.transport,
                enum_text(&record.source.client_surface)?,
                record.source_event_id,
                record.source_sequence.map(|value| value as i64),
                record.source_timestamp,
                record.received_at,
                utc_millis(&record.received_at)?,
                record.receive_sequence as i64,
                record.source_event_type,
                record.external_session_id,
                record.external_turn_id,
                record.external_tool_use_id,
                enum_text(&record.origin)?,
                json(&record.event)?,
                json(&record.sensitivity)?,
                now,
            ],
        )
        .map_err(map_sqlite)?;
    Ok(())
}

fn tombstone_record(
    transaction: &Transaction<'_>,
    record: &SpoolIngressRecord,
    reason: &str,
    now: &str,
    expires_at: &str,
) -> StorageResult<()> {
    insert_tombstone(
        transaction,
        &record.id.to_string(),
        "ingress",
        reason,
        now,
        expires_at,
    )?;
    insert_tombstone(
        transaction,
        &source_identity_key(record),
        "source",
        reason,
        now,
        expires_at,
    )
}

fn insert_tombstone(
    transaction: &Transaction<'_>,
    key: &str,
    kind: &str,
    reason: &str,
    now: &str,
    expires_at: &str,
) -> StorageResult<()> {
    transaction
        .execute(
            "INSERT OR REPLACE INTO ingress_tombstones(
            identity_key, identity_kind, reason, deleted_at, expires_at, expires_at_unix_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![key, kind, reason, now, expires_at, utc_millis(expires_at)?],
        )
        .map_err(map_sqlite)?;
    Ok(())
}

fn insert_rejection(
    transaction: &Transaction<'_>,
    token_hash: &str,
    reason: &str,
    receive_sequence: Option<u64>,
    now: &str,
) -> StorageResult<()> {
    let sequence = receive_sequence
        .map(i64::try_from)
        .transpose()
        .map_err(|_| StorageError::InvalidInput)?;
    transaction.execute(
        "INSERT OR IGNORE INTO ingress_rejections(token_hash, reason, receive_sequence, rejected_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![token_hash, reason, sequence, now],
    ).map_err(map_sqlite)?;
    Ok(())
}

fn record_spool_state(
    transaction: &Transaction<'_>,
    state: &mochi_capture::spool::SpoolState,
    now: &str,
) -> StorageResult<()> {
    let previous: Option<i64> = transaction
        .query_row(
            "SELECT evicted_count FROM import_state WHERE singleton = 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    let count = i64::try_from(state.evicted_count).map_err(|_| StorageError::InvalidInput)?;
    if previous.is_none_or(|value| count > value) && count > 0 {
        let token = format!("spool-state-{count}");
        insert_rejection(transaction, &token, "spool_eviction", None, now)?;
    }
    transaction
        .execute(
            "INSERT INTO import_state(singleton, evicted_count, last_eviction_at, observed_at)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(singleton) DO UPDATE SET
            evicted_count = MAX(import_state.evicted_count, excluded.evicted_count),
            last_eviction_at = COALESCE(excluded.last_eviction_at, import_state.last_eviction_at),
            observed_at = excluded.observed_at",
            params![count, state.last_eviction_at, now],
        )
        .map_err(map_sqlite)?;
    Ok(())
}

fn source_identity_key(record: &SpoolIngressRecord) -> String {
    if record.source_event_id.is_none() {
        return format!("ingress-v1:{}", record.id);
    }
    let mut digest = Sha256::new();
    digest.update(b"mochi-source-identity-v1");
    for value in [
        record.source.provider.as_str(),
        record.source.adapter_version.as_str(),
        record.source.transport.as_str(),
        enum_static(record.source.client_surface),
        record.external_session_id.as_deref().unwrap_or(""),
        record.source_event_id.as_deref().unwrap_or(""),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    format!("source-v1:{:x}", digest.finalize())
}

fn rejection_name(reason: SpoolRejectionReason) -> &'static str {
    match reason {
        SpoolRejectionReason::InvalidFilename => "invalid_filename",
        SpoolRejectionReason::Oversized => "oversized",
        SpoolRejectionReason::Malformed => "malformed",
        SpoolRejectionReason::UnsupportedSchema => "unsupported_schema",
        SpoolRejectionReason::IdentityMismatch => "identity_mismatch",
        SpoolRejectionReason::NotRegularFile => "not_regular_file",
    }
}

fn parse_utc(value: &str) -> StorageResult<()> {
    utc_millis(value).map(|_| ())
}

fn json<T: Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_string(value).map_err(|_| StorageError::Serialization)
}

fn enum_text<T: Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_value(value)
        .map_err(|_| StorageError::Serialization)?
        .as_str()
        .map(str::to_owned)
        .ok_or(StorageError::Serialization)
}

fn enum_static(value: ClientSurface) -> &'static str {
    match value {
        ClientSurface::Cli => "cli",
        ClientSurface::Desktop => "desktop",
        ClientSurface::Unknown => "unknown",
    }
}

pub(crate) type RawIngress = (
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<i64>,
    Option<String>,
    String,
    i64,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    String,
);

pub(crate) fn decode_ingress_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawIngress> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
    ))
}

pub(crate) fn decode_ingress(
    project_id: ProjectId,
    raw: RawIngress,
) -> StorageResult<PersistedIngress> {
    let record = SpoolIngressRecord {
        id: Uuid::parse_str(&raw.0).map_err(|_| StorageError::Corrupt)?,
        project_id: project_id.as_uuid(),
        schema_version: u8::try_from(raw.2).map_err(|_| StorageError::Corrupt)?,
        source: SourceDescriptor {
            provider: raw.3,
            adapter_version: raw.4,
            transport: raw.5,
            client_surface: serde_json::from_value(serde_json::Value::String(raw.6))
                .map_err(|_| StorageError::Corrupt)?,
        },
        source_event_id: raw.7,
        source_sequence: raw
            .8
            .map(u64::try_from)
            .transpose()
            .map_err(|_| StorageError::Corrupt)?,
        source_timestamp: raw.9,
        received_at: raw.10,
        receive_sequence: u64::try_from(raw.11).map_err(|_| StorageError::Corrupt)?,
        source_event_type: raw.12,
        external_session_id: raw.13,
        external_turn_id: raw.14,
        external_tool_use_id: raw.15,
        origin: serde_json::from_value(serde_json::Value::String(raw.16))
            .map_err(|_| StorageError::Corrupt)?,
        event: serde_json::from_str::<NormalizedEvent>(&raw.17)
            .map_err(|_| StorageError::Corrupt)?,
        sensitivity: serde_json::from_str::<Sensitivity>(&raw.18)
            .map_err(|_| StorageError::Corrupt)?,
    };
    validate_record(&record).map_err(|_| StorageError::Corrupt)?;
    Ok(PersistedIngress {
        source_identity_key: raw.1,
        record,
        imported_at: raw.19,
    })
}

impl SqliteStore {
    /// Durable loss metadata, separate from captured provider events.
    pub fn spool_eviction_count(&self) -> StorageResult<u64> {
        let count: Option<i64> = self
            .lock()?
            .query_row(
                "SELECT evicted_count FROM import_state WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sqlite)?;
        u64::try_from(count.unwrap_or(0)).map_err(|_| StorageError::Corrupt)
    }
}
