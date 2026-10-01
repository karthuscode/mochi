use crate::{SqliteStore, StorageError, StorageResult, connection::utc_millis, error::map_sqlite};
use mochi_domain::{
    CaptureCompleteness, CodingSession, CodingSessionData, CommandExecution, FileChange,
    GitContext, GitSnapshot, OverallCompleteness, ProjectId, SessionEvent, SessionId,
    SessionStatus, SessionTurn, ToolExecution, UtcTimestamp,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCursor {
    pub started_at: UtcTimestamp,
    pub session_id: SessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub project_id: ProjectId,
    pub started_at: UtcTimestamp,
    pub ended_at: Option<UtcTimestamp>,
    pub status: SessionStatus,
    pub completeness: OverallCompleteness,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionPage {
    pub items: Vec<SessionSummary>,
    pub next_cursor: Option<SessionCursor>,
}

pub trait CodingSessionRepository {
    fn insert_session(&self, session: &CodingSession) -> StorageResult<()>;
    fn get_session(&self, id: SessionId) -> StorageResult<Option<CodingSession>>;
    fn list_sessions(
        &self,
        project_id: ProjectId,
        before: Option<&SessionCursor>,
        limit: Option<usize>,
    ) -> StorageResult<SessionPage>;
    fn delete_session(&self, id: SessionId) -> StorageResult<bool>;
}

impl CodingSessionRepository for SqliteStore {
    fn insert_session(&self, session: &CodingSession) -> StorageResult<()> {
        let now = self.now()?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        insert_session_transaction(&transaction, session, &now)?;
        transaction.commit().map_err(map_sqlite)
    }

    fn get_session(&self, id: SessionId) -> StorageResult<Option<CodingSession>> {
        let connection = self.lock()?;
        let header = connection
            .query_row(
                "SELECT project_id, schema_version, started_at, ended_at, status,
                        source_json, capture_capabilities_json, capture_completeness_json
                 FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()
            .map_err(map_sqlite)?;
        let Some(header) = header else {
            return Ok(None);
        };
        let turns = load_rows::<SessionTurn>(&connection, "session_turns", id)?;
        let events = load_rows::<SessionEvent>(&connection, "session_events", id)?;
        let tools = load_rows::<ToolExecution>(&connection, "tool_executions", id)?;
        let commands = load_rows::<CommandExecution>(&connection, "command_executions", id)?;
        let files = load_rows::<FileChange>(&connection, "file_changes", id)?;
        let git_context = load_git_context(&connection, id)?;
        let data = CodingSessionData {
            schema_version: u16::try_from(header.1).map_err(|_| StorageError::Corrupt)?,
            id,
            project_id: ProjectId::parse(&header.0).map_err(|_| StorageError::Corrupt)?,
            source: decode_json(&header.5)?,
            started_at: UtcTimestamp::parse(&header.2).map_err(|_| StorageError::Corrupt)?,
            ended_at: header
                .3
                .as_deref()
                .map(UtcTimestamp::parse)
                .transpose()
                .map_err(|_| StorageError::Corrupt)?,
            status: decode_enum(&header.4)?,
            turns,
            events,
            tool_executions: tools,
            command_executions: commands,
            file_changes: files,
            git_context,
            capture_capabilities: decode_json(&header.6)?,
            capture_completeness: decode_json(&header.7)?,
        };
        CodingSession::new(data)
            .map(Some)
            .map_err(|_| StorageError::DomainValidation)
    }

    fn list_sessions(
        &self,
        project_id: ProjectId,
        before: Option<&SessionCursor>,
        limit: Option<usize>,
    ) -> StorageResult<SessionPage> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(StorageError::InvalidInput);
        }
        let before_time = before
            .map(|value| utc_millis(value.started_at.as_str()))
            .transpose()?
            .unwrap_or(i64::MAX);
        let before_id = before
            .map(|value| value.session_id.to_string())
            .unwrap_or_else(|| "ffffffff-ffff-ffff-ffff-ffffffffffff".to_owned());
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, started_at, ended_at, status, capture_completeness_json
                 FROM sessions
                 WHERE project_id = ?1
                   AND (started_at_unix_ms < ?2 OR (started_at_unix_ms = ?2 AND id < ?3))
                 ORDER BY started_at_unix_ms DESC, id DESC LIMIT ?4",
            )
            .map_err(map_sqlite)?;
        let rows = statement
            .query_map(
                params![project_id.to_string(), before_time, before_id, limit as i64],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(map_sqlite)?;
        let mut items = Vec::new();
        for row in rows {
            let row = row.map_err(map_sqlite)?;
            let completeness: CaptureCompleteness = decode_json(&row.4)?;
            items.push(SessionSummary {
                id: SessionId::parse(&row.0).map_err(|_| StorageError::Corrupt)?,
                project_id,
                started_at: UtcTimestamp::parse(&row.1).map_err(|_| StorageError::Corrupt)?,
                ended_at: row
                    .2
                    .as_deref()
                    .map(UtcTimestamp::parse)
                    .transpose()
                    .map_err(|_| StorageError::Corrupt)?,
                status: decode_enum(&row.3)?,
                completeness: completeness.overall(),
            });
        }
        let next_cursor = items.last().map(|item| SessionCursor {
            started_at: item.started_at.clone(),
            session_id: item.id,
        });
        Ok(SessionPage { items, next_cursor })
    }

    fn delete_session(&self, id: SessionId) -> StorageResult<bool> {
        let now = self.now()?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        transaction
            .execute(
                "UPDATE assembly_groups
                 SET state = 'deleted', deleted_at = ?2
                 WHERE session_id = ?1 AND state = 'assembled'",
                params![id.to_string(), now],
            )
            .map_err(map_sqlite)?;
        let deleted = transaction
            .execute("DELETE FROM sessions WHERE id = ?1", [id.to_string()])
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(deleted > 0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionInsertOutcome {
    Inserted,
    Existing,
}

pub(crate) fn insert_session_transaction(
    transaction: &Transaction<'_>,
    session: &CodingSession,
    now: &str,
) -> StorageResult<SessionInsertOutcome> {
    let canonical = session
        .to_json()
        .map_err(|_| StorageError::DomainValidation)?;
    let content_hash = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    let data = session.data();
    let existing: Option<String> = transaction
        .query_row(
            "SELECT content_hash FROM sessions WHERE id = ?1",
            [data.id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_sqlite)?;
    if let Some(existing) = existing {
        return if existing == content_hash {
            Ok(SessionInsertOutcome::Existing)
        } else {
            Err(StorageError::Constraint)
        };
    }
    transaction
        .execute(
            "INSERT INTO sessions(
                id, project_id, schema_version, started_at, started_at_unix_ms, ended_at, status,
                source_json, capture_capabilities_json, capture_completeness_json,
                content_hash, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                data.id.to_string(),
                data.project_id.to_string(),
                i64::from(data.schema_version),
                data.started_at.as_str(),
                utc_millis(data.started_at.as_str())?,
                data.ended_at.as_ref().map(UtcTimestamp::as_str),
                enum_text(&data.status)?,
                json(&data.source)?,
                json(&data.capture_capabilities)?,
                json(&data.capture_completeness)?,
                content_hash,
                now,
            ],
        )
        .map_err(map_sqlite)?;

    insert_rows(
        transaction,
        "session_turns",
        data.id,
        data.turns
            .iter()
            .map(|turn| (turn.id.to_string(), None, None, json(turn)))
            .collect(),
    )?;
    insert_rows(
        transaction,
        "session_events",
        data.id,
        data.events
            .iter()
            .map(|event| {
                (
                    event.id.to_string(),
                    event.turn_id.map(|id| id.to_string()),
                    Some(event.sequence),
                    json(event),
                )
            })
            .collect(),
    )?;
    insert_rows(
        transaction,
        "tool_executions",
        data.id,
        data.tool_executions
            .iter()
            .map(|tool| {
                (
                    tool.id.to_string(),
                    tool.turn_id.map(|id| id.to_string()),
                    None,
                    json(tool),
                )
            })
            .collect(),
    )?;
    insert_commands(transaction, data.id, &data.command_executions)?;
    insert_rows(
        transaction,
        "file_changes",
        data.id,
        data.file_changes
            .iter()
            .map(|change| (change.id.to_string(), None, None, json(change)))
            .collect(),
    )?;
    insert_git_context(transaction, data.id, &data.git_context)?;
    Ok(SessionInsertOutcome::Inserted)
}

type InsertRow = (String, Option<String>, Option<u64>, StorageResult<String>);

fn insert_rows(
    transaction: &Transaction<'_>,
    table: &str,
    session_id: SessionId,
    rows: Vec<InsertRow>,
) -> StorageResult<()> {
    for (position, (id, turn_id, sequence, row_json)) in rows.into_iter().enumerate() {
        let row_json = row_json?;
        let position = i64::try_from(position).map_err(|_| StorageError::InvalidInput)?;
        match table {
            "session_turns" | "file_changes" => {
                transaction.execute(
                    &format!("INSERT INTO {table}(id, session_id, position, row_json) VALUES (?1, ?2, ?3, ?4)"),
                    params![id, session_id.to_string(), position, row_json],
                ).map_err(map_sqlite)?;
            }
            "session_events" => {
                transaction.execute(
                    "INSERT INTO session_events(id, session_id, turn_id, sequence, position, row_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, session_id.to_string(), turn_id, sequence.map(|value| value as i64), position, row_json],
                ).map_err(map_sqlite)?;
            }
            "tool_executions" => {
                transaction
                    .execute(
                        "INSERT INTO tool_executions(id, session_id, turn_id, position, row_json)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![id, session_id.to_string(), turn_id, position, row_json],
                    )
                    .map_err(map_sqlite)?;
            }
            _ => return Err(StorageError::InvalidInput),
        }
    }
    Ok(())
}

fn insert_commands(
    transaction: &Transaction<'_>,
    session_id: SessionId,
    commands: &[CommandExecution],
) -> StorageResult<()> {
    for (position, command) in commands.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO command_executions(
                id, session_id, turn_id, tool_execution_id, position, row_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    command.id.to_string(),
                    session_id.to_string(),
                    command.turn_id.map(|id| id.to_string()),
                    command.tool_execution_id.map(|id| id.to_string()),
                    i64::try_from(position).map_err(|_| StorageError::InvalidInput)?,
                    json(command)?,
                ],
            )
            .map_err(map_sqlite)?;
    }
    Ok(())
}

fn insert_git_context(
    transaction: &Transaction<'_>,
    session_id: SessionId,
    context: &GitContext,
) -> StorageResult<()> {
    match context {
        GitContext::Available { before, after } => {
            insert_git_row(transaction, session_id, "before", before)?;
            if let Some(after) = after {
                insert_git_row(transaction, session_id, "after", after)?;
            }
        }
        GitContext::Unavailable { .. } => {
            insert_git_row(transaction, session_id, "unavailable", context)?
        }
    }
    Ok(())
}

fn insert_git_row<T: Serialize>(
    transaction: &Transaction<'_>,
    session_id: SessionId,
    role: &str,
    value: &T,
) -> StorageResult<()> {
    transaction
        .execute(
            "INSERT INTO git_snapshots(session_id, role, row_json) VALUES (?1, ?2, ?3)",
            params![session_id.to_string(), role, json(value)?],
        )
        .map_err(map_sqlite)?;
    Ok(())
}

fn load_rows<T: DeserializeOwned>(
    connection: &rusqlite::Connection,
    table: &str,
    session_id: SessionId,
) -> StorageResult<Vec<T>> {
    let allowed = [
        "session_turns",
        "session_events",
        "tool_executions",
        "command_executions",
        "file_changes",
    ];
    if !allowed.contains(&table) {
        return Err(StorageError::InvalidInput);
    }
    let mut statement = connection
        .prepare(&format!(
            "SELECT row_json FROM {table} WHERE session_id = ?1 ORDER BY position"
        ))
        .map_err(map_sqlite)?;
    let rows = statement
        .query_map([session_id.to_string()], |row| row.get::<_, String>(0))
        .map_err(map_sqlite)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(decode_json(&row.map_err(map_sqlite)?)?);
    }
    Ok(values)
}

fn load_git_context(
    connection: &rusqlite::Connection,
    session_id: SessionId,
) -> StorageResult<GitContext> {
    let mut statement = connection
        .prepare("SELECT role, row_json FROM git_snapshots WHERE session_id = ?1 ORDER BY role")
        .map_err(map_sqlite)?;
    let rows = statement
        .query_map([session_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(map_sqlite)?;
    let mut before: Option<GitSnapshot> = None;
    let mut after: Option<GitSnapshot> = None;
    let mut unavailable: Option<GitContext> = None;
    for row in rows {
        let (role, value) = row.map_err(map_sqlite)?;
        match role.as_str() {
            "before" => before = Some(decode_json(&value)?),
            "after" => after = Some(decode_json(&value)?),
            "unavailable" => unavailable = Some(decode_json(&value)?),
            _ => return Err(StorageError::Corrupt),
        }
    }
    match (before, unavailable) {
        (Some(before), None) => Ok(GitContext::Available {
            before: Box::new(before),
            after: after.map(Box::new),
        }),
        (None, Some(context @ GitContext::Unavailable { .. })) if after.is_none() => Ok(context),
        _ => Err(StorageError::Corrupt),
    }
}

fn json<T: Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_string(value).map_err(|_| StorageError::Serialization)
}

fn decode_json<T: DeserializeOwned>(value: &str) -> StorageResult<T> {
    serde_json::from_str(value).map_err(|_| StorageError::Corrupt)
}

fn enum_text<T: Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_value(value)
        .map_err(|_| StorageError::Serialization)?
        .as_str()
        .map(str::to_owned)
        .ok_or(StorageError::Serialization)
}

fn decode_enum<T: DeserializeOwned>(value: &str) -> StorageResult<T> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .map_err(|_| StorageError::Corrupt)
}
