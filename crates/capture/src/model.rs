use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const INGRESS_SCHEMA_VERSION: u8 = 1;
pub const SANITIZER_RULES_VERSION: &str = mochi_privacy::REDACTION_RULES_VERSION;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientSurface {
    Cli,
    Desktop,
    Unknown,
}

impl ClientSurface {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cli" => Some(Self::Cli),
            "desktop" => Some(Self::Desktop),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSourceCapabilities {
    pub identity: CapabilitySupport,
    pub project_association: CapabilitySupport,
    pub activity: CapabilitySupport,
    pub prompts: CapabilitySupport,
    pub agent_messages: CapabilitySupport,
    pub tool_lifecycle: CapabilitySupport,
    pub permissions: CapabilitySupport,
    pub interrupts: CapabilitySupport,
    pub commands: CapabilitySupport,
    pub file_events: CapabilitySupport,
    pub session_end: CapabilitySupport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceDescriptor {
    pub provider: String,
    pub adapter_version: String,
    pub transport: String,
    pub client_surface: ClientSurface,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityClassification {
    Sanitized,
    MetadataOnly,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpoolIngressRecord {
    pub schema_version: u8,
    pub id: Uuid,
    pub project_id: Uuid,
    pub source: SourceDescriptor,
    pub source_event_id: Option<String>,
    pub source_sequence: Option<u64>,
    pub source_timestamp: Option<String>,
    pub received_at: String,
    pub receive_sequence: u64,
    pub source_event_type: String,
    pub external_session_id: Option<String>,
    pub external_turn_id: Option<String>,
    pub external_tool_use_id: Option<String>,
    pub origin: EventOrigin,
    #[serde(flatten)]
    pub event: NormalizedEvent,
    pub sensitivity: Sensitivity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventOrigin {
    Provider,
    Git,
    Mochi,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "eventType", content = "payload")]
pub enum NormalizedEvent {
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
    #[serde(rename = "command.executed")]
    CommandExecuted(CommandExecutedPayload),
    #[serde(rename = "command.result")]
    CommandResult(CommandResultPayload),
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
    pub turn_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentMessagePayload {
    pub text: String,
    pub turn_ref: Option<String>,
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
    pub turn_ref: Option<String>,
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
    pub turn_ref: Option<String>,
    pub summary: Option<String>,
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
pub struct TurnCompletedPayload {
    pub turn_ref: Option<String>,
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
    pub turn_ref: Option<String>,
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
    pub from: Option<String>,
    pub to: Option<String>,
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

#[derive(Clone, Debug)]
pub struct PendingEvent {
    pub source_event_id: Option<String>,
    pub source_event_type: String,
    pub external_session_id: Option<String>,
    pub external_turn_id: Option<String>,
    pub external_tool_use_id: Option<String>,
    pub event: NormalizedEvent,
    pub sensitivity: Sensitivity,
}
