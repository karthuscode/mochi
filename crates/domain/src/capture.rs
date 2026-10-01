use crate::{EventId, ProjectId, SessionId, TurnId, UtcTimestamp};
use serde::{Deserialize, Serialize};

pub const SESSION_EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    Partial,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCapabilitySupport {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceCompleteness {
    Complete,
    Partial,
    Missing,
    Unknown,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverallCompleteness {
    Complete,
    Partial,
    Degraded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureCapabilities {
    pub session_lifecycle: CapabilitySupport,
    pub user_prompt: CapabilitySupport,
    pub agent_response: CapabilitySupport,
    pub tool_activity: CapabilitySupport,
    pub commands: CapabilitySupport,
    pub file_activity: CapabilitySupport,
    pub permissions: CapabilitySupport,
    pub interrupts: CapabilitySupport,
    pub git_context: CapabilitySupport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSourceCapabilities {
    pub identity: SourceCapabilitySupport,
    pub project_association: SourceCapabilitySupport,
    pub activity: SourceCapabilitySupport,
    pub prompts: SourceCapabilitySupport,
    pub agent_messages: SourceCapabilitySupport,
    pub tool_lifecycle: SourceCapabilitySupport,
    pub permissions: SourceCapabilitySupport,
    pub interrupts: SourceCapabilitySupport,
    pub commands: SourceCapabilitySupport,
    pub file_events: SourceCapabilitySupport,
    pub session_end: SourceCapabilitySupport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureCompleteness {
    pub session_lifecycle: EvidenceCompleteness,
    pub prompt: EvidenceCompleteness,
    pub agent_response: EvidenceCompleteness,
    pub tools: EvidenceCompleteness,
    pub commands: EvidenceCompleteness,
    pub files: EvidenceCompleteness,
    pub git: EvidenceCompleteness,
    pub interrupts: EvidenceCompleteness,
}

impl CaptureCompleteness {
    pub fn overall(&self) -> OverallCompleteness {
        let dimensions = [
            self.session_lifecycle,
            self.prompt,
            self.agent_response,
            self.tools,
            self.commands,
            self.files,
            self.git,
            self.interrupts,
        ];
        if dimensions.contains(&EvidenceCompleteness::Missing) {
            OverallCompleteness::Degraded
        } else if dimensions.iter().all(|value| {
            matches!(
                value,
                EvidenceCompleteness::Complete | EvidenceCompleteness::NotApplicable
            )
        }) {
            OverallCompleteness::Complete
        } else {
            OverallCompleteness::Partial
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientSurface {
    Cli,
    Desktop,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDescriptor {
    pub provider: String,
    pub client_surface: ClientSurface,
    pub client_version: Option<String>,
    pub adapter_version: String,
    pub transport: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventOrigin {
    Provider,
    Git,
    Mochi,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventProvenance {
    pub source_event_id: Option<String>,
    pub source_sequence: Option<u64>,
    pub source_event_type: Option<String>,
    pub occurred_at: Option<UtcTimestamp>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityClassification {
    Sanitized,
    MetadataOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sensitivity {
    pub classification: SensitivityClassification,
    pub redaction_count: u32,
    pub rules_version: String,
    pub policy_revision: u64,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionEvent {
    pub schema_version: u16,
    pub id: EventId,
    pub session_id: SessionId,
    pub project_id: ProjectId,
    pub turn_id: Option<TurnId>,
    pub source: SourceDescriptor,
    pub provenance: EventProvenance,
    pub received_at: UtcTimestamp,
    pub sequence: u64,
    pub origin: EventOrigin,
    #[serde(flatten)]
    pub event: SessionEventData,
    pub sensitivity: Sensitivity,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "eventType", content = "payload")]
pub enum SessionEventData {
    #[serde(rename = "session.started")]
    SessionStarted(SessionStartedPayload),
    #[serde(rename = "user.prompt")]
    UserPrompt(UserPromptPayload),
    #[serde(rename = "agent.message")]
    AgentMessage(AgentMessagePayload),
    #[serde(rename = "tool.started")]
    ToolStarted(ToolStartedPayload),
    #[serde(rename = "tool.completed")]
    ToolCompleted(ToolCompletedPayload),
    #[serde(rename = "permission.requested")]
    PermissionRequested(PermissionRequestedPayload),
    #[serde(rename = "file.changed")]
    FileChanged(FileChangedPayload),
    #[serde(rename = "command.executed")]
    CommandExecuted(CommandExecutedPayload),
    #[serde(rename = "command.result")]
    CommandResult(CommandResultPayload),
    #[serde(rename = "test.result")]
    TestResult(TestResultPayload),
    #[serde(rename = "error.encountered")]
    ErrorEncountered(ErrorEncounteredPayload),
    #[serde(rename = "turn.completed")]
    TurnCompleted(TurnCompletedPayload),
    #[serde(rename = "session.stopped")]
    SessionStopped(SessionStoppedPayload),
    #[serde(rename = "context.compacted")]
    ContextCompacted(ContextCompactedPayload),
    #[serde(rename = "capture.gap")]
    CaptureGap(CaptureGapPayload),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionStartedPayload {
    pub reason: SessionStartReason,
    pub capabilities: SessionSourceCapabilities,
    pub start_boundary_known: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartReason {
    Startup,
    Resume,
    Observed,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserPromptPayload {
    pub text: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentMessagePayload {
    pub text: String,
    pub message_kind: MessageKind,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Progress,
    Final,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolStartedPayload {
    pub tool_call_ref: String,
    pub tool_kind: String,
    pub summary: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCompletedPayload {
    pub tool_call_ref: String,
    pub status: CompletionStatus,
    pub summary: Option<String>,
    pub duration_ms: Option<u64>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    Succeeded,
    Failed,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionRequestedPayload {
    pub tool_kind: String,
    pub tool_call_ref: Option<String>,
    pub summary: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileChangedPayload {
    pub path: String,
    pub change: FileChangeKind,
    pub old_path: Option<String>,
    pub evidence_ref: Option<String>,
    pub attribution: FileAttribution,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Unknown,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAttribution {
    ProviderReported,
    Observed,
    Ambiguous,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandExecutedPayload {
    pub command_ref: String,
    pub display: String,
    pub working_directory: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandResultPayload {
    pub command_ref: String,
    pub exit_code: Option<i32>,
    pub output: Option<String>,
    pub duration_ms: Option<u64>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TestResultPayload {
    pub run_ref: String,
    pub status: CompletionStatus,
    pub passed_count: Option<u32>,
    pub failed_count: Option<u32>,
    pub summary: Option<String>,
    pub evidence_ref: Option<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ErrorEncounteredPayload {
    pub category: ErrorCategory,
    pub summary: String,
    pub related_event_id: Option<EventId>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Tool,
    Test,
    Integration,
    Runtime,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurnCompletedPayload {
    pub status: CompletionStatus,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionStoppedPayload {
    pub reason: SessionStopReason,
    pub end_boundary_known: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStopReason {
    Completed,
    UserFinalized,
    Interrupted,
    IdleConfirmed,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextCompactedPayload {
    pub phase: CompactionPhase,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionPhase {
    Before,
    After,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureGapPayload {
    pub reason: CaptureGapReason,
    pub from: Option<UtcTimestamp>,
    pub to: Option<UtcTimestamp>,
    pub dropped_count: Option<u64>,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureGapReason {
    MissingStart,
    MissingEnd,
    Paused,
    Overflow,
    Corrupt,
    Unsupported,
    Restart,
    AmbiguousAttribution,
    InvalidInput,
    SanitizationFailed,
}
