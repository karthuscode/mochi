use crate::{
    CaptureCapabilities, CaptureCompleteness, CommandExecutionId, DomainError, DomainResult,
    EventId, FileAttribution, FileChangeId, FileChangeKind, GitContext, OverallCompleteness,
    ProjectId, SESSION_EVENT_SCHEMA_VERSION, SessionEvent, SessionId, SourceDescriptor,
    ToolExecutionId, TurnId, UtcTimestamp,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

pub const CODING_SESSION_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryKind {
    Git,
    Other,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepositoryIdentity {
    pub kind: RepositoryKind,
    pub opaque_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: ProjectId,
    pub display_name: String,
    pub root_path: String,
    pub repository_identity: Option<RepositoryIdentity>,
    pub created_at: UtcTimestamp,
    pub last_seen_at: UtcTimestamp,
}

impl Project {
    pub fn validate(&self) -> DomainResult<()> {
        if self.display_name.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "project.displayName",
                reason: "must not be empty",
            });
        }
        if self.root_path.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "project.rootPath",
                reason: "must not be empty",
            });
        }
        if self.created_at > self.last_seen_at {
            return Err(DomainError::InvalidTimeRange { entity: "project" });
        }
        if self
            .repository_identity
            .as_ref()
            .is_some_and(|identity| identity.opaque_id.trim().is_empty())
        {
            return Err(DomainError::InvalidValue {
                field: "project.repositoryIdentity.opaqueId",
                reason: "must not be empty",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Active,
    Completed,
    Interrupted,
    Incomplete,
    Failed,
}

impl SessionStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        self == next || matches!(self, Self::Active) && !matches!(next, Self::Active)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Active,
    Completed,
    Interrupted,
    Incomplete,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionTurn {
    pub id: TurnId,
    pub session_id: SessionId,
    pub source_turn_id: Option<String>,
    pub started_at: Option<UtcTimestamp>,
    pub ended_at: Option<UtcTimestamp>,
    pub user_prompt: Option<String>,
    pub agent_final_response: Option<String>,
    pub event_ids: Vec<EventId>,
    pub status: TurnStatus,
}

impl SessionTurn {
    pub fn validate(&self) -> DomainResult<()> {
        validate_time_range("turn", self.started_at.as_ref(), self.ended_at.as_ref())?;
        if self.status == TurnStatus::Completed && self.ended_at.is_none() {
            return Err(DomainError::InvalidStatus {
                entity: "turn",
                reason: "completed turns require endedAt",
            });
        }
        if self.status == TurnStatus::Active && self.ended_at.is_some() {
            return Err(DomainError::InvalidStatus {
                entity: "turn",
                reason: "active turns cannot have endedAt",
            });
        }
        ensure_unique(&self.event_ids, "turn", "contains duplicate event IDs")
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Started,
    Completed,
    Failed,
    Interrupted,
    Incomplete,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolExecution {
    pub id: ToolExecutionId,
    pub session_id: SessionId,
    pub turn_id: Option<TurnId>,
    pub source_tool_use_id: Option<String>,
    pub tool_name: String,
    pub started_at: Option<UtcTimestamp>,
    pub completed_at: Option<UtcTimestamp>,
    pub status: ToolExecutionStatus,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub related_file_paths: Vec<String>,
}

impl ToolExecution {
    pub fn validate(&self) -> DomainResult<()> {
        if self.tool_name.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "toolExecution.toolName",
                reason: "must not be empty",
            });
        }
        validate_time_range(
            "tool execution",
            self.started_at.as_ref(),
            self.completed_at.as_ref(),
        )?;
        if matches!(
            self.status,
            ToolExecutionStatus::Completed | ToolExecutionStatus::Failed
        ) && self.completed_at.is_none()
        {
            return Err(DomainError::InvalidStatus {
                entity: "tool execution",
                reason: "completed or failed tools require completedAt",
            });
        }
        for path in &self.related_file_paths {
            crate::git::validate_relative_path(path)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandExecutionStatus {
    Started,
    Completed,
    Failed,
    Interrupted,
    Incomplete,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandExecution {
    pub id: CommandExecutionId,
    pub session_id: SessionId,
    pub turn_id: Option<TurnId>,
    pub tool_execution_id: Option<ToolExecutionId>,
    pub command: String,
    pub working_directory: Option<String>,
    pub started_at: Option<UtcTimestamp>,
    pub completed_at: Option<UtcTimestamp>,
    pub status: CommandExecutionStatus,
    pub exit_code: Option<i32>,
    pub output_summary: Option<String>,
}

impl CommandExecution {
    pub fn validate(&self) -> DomainResult<()> {
        if self.command.trim().is_empty() {
            return Err(DomainError::InvalidValue {
                field: "commandExecution.command",
                reason: "must not be empty",
            });
        }
        validate_time_range(
            "command execution",
            self.started_at.as_ref(),
            self.completed_at.as_ref(),
        )?;
        if matches!(
            self.status,
            CommandExecutionStatus::Completed | CommandExecutionStatus::Failed
        ) && self.completed_at.is_none()
        {
            return Err(DomainError::InvalidStatus {
                entity: "command execution",
                reason: "completed or failed commands require completedAt",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeSource {
    ProviderEvent,
    GitComparison,
    Derived,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChange {
    pub id: FileChangeId,
    pub path: String,
    pub previous_path: Option<String>,
    pub change_type: FileChangeKind,
    pub source: FileChangeSource,
    pub observed_during_session: bool,
    pub attribution: FileAttribution,
}

impl FileChange {
    pub fn validate(&self) -> DomainResult<()> {
        crate::git::validate_relative_path(&self.path)?;
        if let Some(path) = &self.previous_path {
            crate::git::validate_relative_path(path)?;
        }
        if self.change_type == FileChangeKind::Renamed && self.previous_path.is_none() {
            return Err(DomainError::InvalidValue {
                field: "fileChange.previousPath",
                reason: "renamed files require a previous path",
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodingSessionData {
    pub schema_version: u16,
    pub id: SessionId,
    pub project_id: ProjectId,
    pub source: SourceDescriptor,
    pub started_at: UtcTimestamp,
    pub ended_at: Option<UtcTimestamp>,
    pub status: SessionStatus,
    pub turns: Vec<SessionTurn>,
    pub events: Vec<SessionEvent>,
    pub tool_executions: Vec<ToolExecution>,
    pub command_executions: Vec<CommandExecution>,
    pub file_changes: Vec<FileChange>,
    pub git_context: GitContext,
    pub capture_capabilities: CaptureCapabilities,
    pub capture_completeness: CaptureCompleteness,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CodingSession(CodingSessionData);

impl CodingSession {
    pub fn new(data: CodingSessionData) -> DomainResult<Self> {
        validate_session(&data)?;
        Ok(Self(data))
    }

    pub fn data(&self) -> &CodingSessionData {
        &self.0
    }

    pub fn into_data(self) -> CodingSessionData {
        self.0
    }

    pub fn overall_completeness(&self) -> OverallCompleteness {
        self.0.capture_completeness.overall()
    }

    pub fn to_json(&self) -> DomainResult<String> {
        serde_json::to_string(self).map_err(|error| DomainError::Serialization(error.to_string()))
    }

    pub fn from_json(value: &str) -> DomainResult<Self> {
        serde_json::from_str(value).map_err(|error| DomainError::Serialization(error.to_string()))
    }

    pub fn transition_status(&self, next: SessionStatus) -> DomainResult<()> {
        if self.0.status.can_transition_to(next) {
            Ok(())
        } else {
            Err(DomainError::InvalidStatusTransition)
        }
    }
}

impl<'de> Deserialize<'de> for CodingSession {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = CodingSessionData::deserialize(deserializer)?;
        Self::new(data).map_err(serde::de::Error::custom)
    }
}

fn validate_session(data: &CodingSessionData) -> DomainResult<()> {
    if data.schema_version != CODING_SESSION_SCHEMA_VERSION {
        return Err(DomainError::UnsupportedSchemaVersion {
            found: data.schema_version,
        });
    }
    validate_time_range("session", Some(&data.started_at), data.ended_at.as_ref())?;
    match data.status {
        SessionStatus::Active if data.ended_at.is_some() => {
            return Err(DomainError::InvalidStatus {
                entity: "session",
                reason: "active sessions cannot have endedAt",
            });
        }
        SessionStatus::Completed | SessionStatus::Interrupted | SessionStatus::Failed
            if data.ended_at.is_none() =>
        {
            return Err(DomainError::InvalidStatus {
                entity: "session",
                reason: "completed, interrupted, and failed sessions require endedAt",
            });
        }
        _ => {}
    }
    if data.source.provider.trim().is_empty()
        || data.source.adapter_version.trim().is_empty()
        || data.source.transport.trim().is_empty()
    {
        return Err(DomainError::InvalidValue {
            field: "session.source",
            reason: "provider, adapterVersion, and transport must not be empty",
        });
    }

    let mut event_ids = HashSet::new();
    let mut sequences = HashSet::new();
    for event in &data.events {
        if event.schema_version != SESSION_EVENT_SCHEMA_VERSION {
            return Err(DomainError::UnsupportedSchemaVersion {
                found: event.schema_version,
            });
        }
        if event.session_id != data.id || event.project_id != data.project_id {
            return Err(DomainError::InvalidReference {
                entity: "event",
                reason: "session or project ID does not match aggregate",
            });
        }
        if event.sequence == 0 || !sequences.insert(event.sequence) {
            return Err(DomainError::InvalidReference {
                entity: "event",
                reason: "receive sequences must be positive and unique",
            });
        }
        if !event_ids.insert(event.id) {
            return Err(DomainError::InvalidReference {
                entity: "event",
                reason: "event IDs must be unique",
            });
        }
    }

    let turn_ids: HashSet<_> = data.turns.iter().map(|turn| turn.id).collect();
    if turn_ids.len() != data.turns.len() {
        return Err(DomainError::InvalidReference {
            entity: "turn",
            reason: "turn IDs must be unique",
        });
    }
    for turn in &data.turns {
        turn.validate()?;
        if turn.session_id != data.id || turn.event_ids.iter().any(|id| !event_ids.contains(id)) {
            return Err(DomainError::InvalidReference {
                entity: "turn",
                reason: "session or event reference does not match aggregate",
            });
        }
    }
    for event in &data.events {
        if event.turn_id.is_some_and(|id| !turn_ids.contains(&id)) {
            return Err(DomainError::InvalidReference {
                entity: "event",
                reason: "turn reference does not match aggregate",
            });
        }
    }

    let tool_ids: HashSet<_> = data.tool_executions.iter().map(|tool| tool.id).collect();
    if tool_ids.len() != data.tool_executions.len() {
        return Err(DomainError::InvalidReference {
            entity: "tool execution",
            reason: "IDs must be unique",
        });
    }
    for tool in &data.tool_executions {
        tool.validate()?;
        validate_session_turn_ref(
            data.id,
            tool.session_id,
            tool.turn_id,
            &turn_ids,
            "tool execution",
        )?;
    }
    for command in &data.command_executions {
        command.validate()?;
        validate_session_turn_ref(
            data.id,
            command.session_id,
            command.turn_id,
            &turn_ids,
            "command execution",
        )?;
        if command
            .tool_execution_id
            .is_some_and(|id| !tool_ids.contains(&id))
        {
            return Err(DomainError::InvalidReference {
                entity: "command execution",
                reason: "tool execution reference does not match aggregate",
            });
        }
    }
    ensure_unique(
        &data
            .command_executions
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>(),
        "command execution",
        "IDs must be unique",
    )?;
    ensure_unique(
        &data
            .file_changes
            .iter()
            .map(|value| value.id)
            .collect::<Vec<_>>(),
        "file change",
        "IDs must be unique",
    )?;
    for change in &data.file_changes {
        change.validate()?;
    }
    data.git_context.validate()?;
    Ok(())
}

fn validate_session_turn_ref(
    session: SessionId,
    entity_session: SessionId,
    turn: Option<TurnId>,
    turns: &HashSet<TurnId>,
    entity: &'static str,
) -> DomainResult<()> {
    if entity_session != session || turn.is_some_and(|id| !turns.contains(&id)) {
        return Err(DomainError::InvalidReference {
            entity,
            reason: "session or turn reference does not match aggregate",
        });
    }
    Ok(())
}

fn validate_time_range(
    entity: &'static str,
    start: Option<&UtcTimestamp>,
    end: Option<&UtcTimestamp>,
) -> DomainResult<()> {
    if let (Some(start), Some(end)) = (start, end)
        && start > end
    {
        return Err(DomainError::InvalidTimeRange { entity });
    }
    Ok(())
}

fn ensure_unique<T: Eq + std::hash::Hash>(
    values: &[T],
    entity: &'static str,
    reason: &'static str,
) -> DomainResult<()> {
    let mut seen = HashSet::new();
    if values.iter().any(|value| !seen.insert(value)) {
        return Err(DomainError::InvalidReference { entity, reason });
    }
    Ok(())
}
