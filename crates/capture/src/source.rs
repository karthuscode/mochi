use crate::model::{
    AgentMessagePayload, CapabilitySupport, CaptureGapPayload, CaptureGapReason, ClientSurface,
    CommandExecutedPayload, CommandResultPayload, CompactionPhase, CompletionStatus,
    ContextCompactedPayload, MessageKind, NormalizedEvent, PendingEvent,
    PermissionRequestedPayload, Sensitivity, SensitivityClassification, SessionSourceCapabilities,
    SessionStartReason, SessionStartedPayload, SessionStopReason, SessionStoppedPayload,
    SourceDescriptor, ToolCompletedPayload, ToolStartedPayload, TurnCompletedPayload,
    UserPromptPayload, SANITIZER_RULES_VERSION,
};
use crate::sanitize::{ContentSanitizer, SanitizeError, SanitizedText, MAX_TEXT_BYTES};
use mochi_privacy::{Decision, FilePolicy, PolicyError, RedactionEngine, Source};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

pub const MAX_PROVIDER_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_REFERENCE_BYTES: usize = 256;

pub trait Clock: Send + Sync {
    fn now_rfc3339(&self) -> String;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_rfc3339(&self) -> String {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
    }
}

pub trait SessionSource {
    fn descriptor(&self) -> SourceDescriptor;
    fn capabilities(&self) -> SessionSourceCapabilities;
    fn normalize(&self, raw: &[u8]) -> Result<Vec<PendingEvent>, IntakeError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntakeError {
    Oversized,
    Malformed,
    MissingRequired(&'static str),
    InvalidRequired(&'static str),
    OutsideApprovedRoot,
    SanitizationFailed,
}

#[derive(Debug, Deserialize)]
struct CodexHookPayload {
    hook_event_name: String,
    session_id: Option<String>,
    cwd: Option<String>,
    turn_id: Option<String>,
    tool_use_id: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<Value>,
    tool_response: Option<Value>,
    prompt: Option<String>,
    last_assistant_message: Option<String>,
    source: Option<String>,
    #[allow(dead_code)]
    reason: Option<String>,
}

pub struct CodexSessionSource<S, C> {
    project_id: Uuid,
    approved_root: PathBuf,
    client_surface: ClientSurface,
    policy_revision: u64,
    sanitizer: S,
    clock: C,
    file_policy: Result<FilePolicy, PolicyError>,
}

impl<S: ContentSanitizer, C: Clock> CodexSessionSource<S, C> {
    pub fn new(
        project_id: Uuid,
        approved_root: PathBuf,
        client_surface: ClientSurface,
        policy_revision: u64,
        sanitizer: S,
        clock: C,
    ) -> Self {
        let approved_root = approved_root.canonicalize().unwrap_or(approved_root);
        let file_policy = FilePolicy::load(&approved_root);
        Self {
            project_id,
            approved_root,
            client_surface,
            policy_revision,
            sanitizer,
            clock,
            file_policy,
        }
    }

    pub fn project_id(&self) -> Uuid {
        self.project_id
    }

    pub fn metadata_gap(&self, error: &IntakeError) -> PendingEvent {
        let reason = match error {
            IntakeError::Oversized => CaptureGapReason::Overflow,
            IntakeError::SanitizationFailed => CaptureGapReason::SanitizationFailed,
            IntakeError::Malformed
            | IntakeError::MissingRequired(_)
            | IntakeError::InvalidRequired(_)
            | IntakeError::OutsideApprovedRoot => CaptureGapReason::InvalidInput,
        };
        PendingEvent {
            source_event_id: None,
            source_event_type: "invalid".to_owned(),
            external_session_id: None,
            external_turn_id: None,
            external_tool_use_id: None,
            event: NormalizedEvent::CaptureGap(CaptureGapPayload {
                reason,
                from: None,
                to: None,
                dropped_count: Some(1),
            }),
            sensitivity: self.metadata_sensitivity(),
        }
    }

    pub fn received_at(&self) -> String {
        self.clock.now_rfc3339()
    }

    fn validate_ref(value: Option<String>, required: bool) -> Result<Option<String>, IntakeError> {
        let Some(value) = value else {
            return if required {
                Err(IntakeError::MissingRequired("reference"))
            } else {
                Ok(None)
            };
        };
        if value.is_empty()
            || value.len() > MAX_REFERENCE_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_:.".contains(&byte))
        {
            return Err(IntakeError::InvalidRequired("reference"));
        }
        Ok(Some(value))
    }

    fn validate_cwd(&self, cwd: Option<String>) -> Result<(), IntakeError> {
        let cwd = cwd.ok_or(IntakeError::MissingRequired("cwd"))?;
        let canonical = Path::new(&cwd)
            .canonicalize()
            .map_err(|_| IntakeError::InvalidRequired("cwd"))?;
        if canonical != self.approved_root && !canonical.starts_with(&self.approved_root) {
            return Err(IntakeError::OutsideApprovedRoot);
        }
        Ok(())
    }

    fn metadata_sensitivity(&self) -> Sensitivity {
        Sensitivity {
            classification: SensitivityClassification::MetadataOnly,
            redaction_count: 0,
            rules_version: SANITIZER_RULES_VERSION.to_owned(),
            policy_revision: self.policy_revision,
            truncated: false,
        }
    }

    fn content_sensitivity(&self, text: &SanitizedText) -> Sensitivity {
        Sensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: text.redaction_count,
            rules_version: SANITIZER_RULES_VERSION.to_owned(),
            policy_revision: self.policy_revision,
            truncated: text.truncated,
        }
    }

    fn sanitize(&self, value: &str) -> Result<SanitizedText, IntakeError> {
        self.sanitizer
            .sanitize_text(value)
            .map_err(|_| IntakeError::SanitizationFailed)
    }

    fn sanitize_json(&self, value: &Value) -> Result<SanitizedText, IntakeError> {
        let redactor = RedactionEngine::new().map_err(|_| IntakeError::SanitizationFailed)?;
        let sanitized = redactor
            .redact_value(value)
            .map_err(|_| IntakeError::SanitizationFailed)?;
        let mut redactions = sanitized
            .findings
            .iter()
            .fold(0u32, |total, item| total.saturating_add(item.count));
        let mut truncated = false;
        let masked = self.mask_json_strings(&sanitized.value, &mut redactions, &mut truncated)?;
        let mut serialized =
            serde_json::to_string(&masked).map_err(|_| IntakeError::SanitizationFailed)?;
        if serialized.len() > MAX_TEXT_BYTES {
            let mut end = MAX_TEXT_BYTES;
            while end > 0 && !serialized.is_char_boundary(end) {
                end -= 1;
            }
            serialized.truncate(end);
            serialized.push_str("[TRUNCATED]");
            truncated = true;
        }
        Ok(SanitizedText {
            value: serialized,
            redaction_count: redactions,
            truncated,
        })
    }

    fn mask_json_strings(
        &self,
        value: &Value,
        redactions: &mut u32,
        truncated: &mut bool,
    ) -> Result<Value, IntakeError> {
        match value {
            Value::String(text) => {
                let result = self.sanitize(text)?;
                *redactions = redactions.saturating_add(result.redaction_count);
                *truncated |= result.truncated;
                Ok(Value::String(result.value))
            }
            Value::Array(items) => items
                .iter()
                .map(|item| self.mask_json_strings(item, redactions, truncated))
                .collect(),
            Value::Object(items) => {
                let mut output = Map::new();
                for (key, item) in items {
                    output.insert(
                        key.clone(),
                        self.mask_json_strings(item, redactions, truncated)?,
                    );
                }
                Ok(Value::Object(output))
            }
            _ => Ok(value.clone()),
        }
    }

    fn command_text(&self, input: Option<&Value>) -> Result<Option<SanitizedText>, IntakeError> {
        let Some(Value::Object(object)) = input else {
            return Ok(None);
        };
        let Some(command) = object.get("command") else {
            return Ok(None);
        };
        match command {
            Value::String(value) => self.sanitize(value).map(Some),
            Value::Array(values) if values.iter().all(Value::is_string) => {
                let joined = values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ");
                self.sanitize(&joined).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Provider file payloads are retained only when every identified path is allowed.
    /// Unidentified non-command tool output is metadata-only because it can contain file text.
    fn file_tool_content_allowed(&self, input: Option<&Value>, response: Option<&Value>) -> bool {
        let Ok(policy) = &self.file_policy else {
            return false;
        };
        let Some(input) = input else {
            return false;
        };
        let mut paths = Vec::new();
        collect_file_paths(input, &mut paths, 0)
            && response.is_none_or(|value| collect_file_paths(value, &mut paths, 0))
            && !paths.is_empty()
            && paths.iter().all(|path| {
                let first = policy.evaluate(Path::new(path), None, Source::Capture);
                let Some(relative) = first.safe_path.as_deref() else {
                    return false;
                };
                if first.decision != Decision::Allow {
                    return false;
                }
                let Ok(metadata) = std::fs::metadata(policy.root().join(relative)) else {
                    return false;
                };
                metadata.is_file()
                    && policy
                        .evaluate(Path::new(path), Some(metadata.len()), Source::Capture)
                        .decision
                        == Decision::Allow
            })
    }

    fn stable_id(parts: &[&str]) -> String {
        parts.join(":")
    }

    fn pending(
        &self,
        hook: &str,
        session: &str,
        references: (Option<&str>, Option<&str>),
        stable_id: Option<String>,
        event: NormalizedEvent,
        sensitivity: Sensitivity,
    ) -> PendingEvent {
        let (turn, tool) = references;
        PendingEvent {
            source_event_id: stable_id,
            source_event_type: hook.to_owned(),
            external_session_id: Some(session.to_owned()),
            external_turn_id: turn.map(ToOwned::to_owned),
            external_tool_use_id: tool.map(ToOwned::to_owned),
            event,
            sensitivity,
        }
    }
}

impl<S: ContentSanitizer, C: Clock> SessionSource for CodexSessionSource<S, C> {
    fn descriptor(&self) -> SourceDescriptor {
        SourceDescriptor {
            provider: "codex".to_owned(),
            adapter_version: env!("CARGO_PKG_VERSION").to_owned(),
            transport: "hooks".to_owned(),
            client_surface: self.client_surface,
        }
    }

    fn capabilities(&self) -> SessionSourceCapabilities {
        let prompt_support = match self.client_surface {
            ClientSurface::Cli => CapabilitySupport::Supported,
            ClientSurface::Desktop | ClientSurface::Unknown => CapabilitySupport::Unknown,
        };
        let interrupt_support = match self.client_surface {
            ClientSurface::Cli => CapabilitySupport::Supported,
            ClientSurface::Desktop | ClientSurface::Unknown => CapabilitySupport::Unknown,
        };
        SessionSourceCapabilities {
            identity: CapabilitySupport::Supported,
            project_association: CapabilitySupport::Supported,
            activity: CapabilitySupport::Supported,
            prompts: prompt_support,
            agent_messages: CapabilitySupport::Supported,
            tool_lifecycle: CapabilitySupport::Supported,
            permissions: CapabilitySupport::Supported,
            interrupts: interrupt_support,
            commands: CapabilitySupport::Supported,
            file_events: CapabilitySupport::Unknown,
            session_end: CapabilitySupport::Supported,
        }
    }

    fn normalize(&self, raw: &[u8]) -> Result<Vec<PendingEvent>, IntakeError> {
        if raw.len() > MAX_PROVIDER_MESSAGE_BYTES {
            return Err(IntakeError::Oversized);
        }
        if raw.is_empty() {
            return Err(IntakeError::Malformed);
        }
        let payload: CodexHookPayload = serde_json::from_value(
            mochi_privacy::parse_bounded_json(raw, MAX_PROVIDER_MESSAGE_BYTES)
                .map_err(|_| IntakeError::Malformed)?,
        )
        .map_err(|_| IntakeError::Malformed)?;
        if payload.hook_event_name.is_empty() || payload.hook_event_name.len() > 64 {
            return Err(IntakeError::InvalidRequired("hook_event_name"));
        }
        self.validate_cwd(payload.cwd)?;
        let session = Self::validate_ref(payload.session_id, true)?
            .ok_or(IntakeError::MissingRequired("session_id"))?;
        let turn = Self::validate_ref(payload.turn_id, false)?;
        let tool = Self::validate_ref(payload.tool_use_id, false)?;
        let hook = payload.hook_event_name.as_str();
        let mut events = Vec::new();

        match hook {
            "SessionStart" => {
                let reason = match payload.source.as_deref() {
                    Some("startup") => SessionStartReason::Startup,
                    Some("resume") => SessionStartReason::Resume,
                    _ => SessionStartReason::Observed,
                };
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    None,
                    NormalizedEvent::SessionStarted(SessionStartedPayload {
                        reason,
                        capabilities: self.capabilities(),
                        start_boundary_known: true,
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "UserPromptSubmit" => {
                let text = payload
                    .prompt
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("prompt"))?;
                let text = self.sanitize(text)?;
                let source_id = turn
                    .as_deref()
                    .map(|turn| Self::stable_id(&["UserPromptSubmit", &session, turn]));
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    source_id,
                    NormalizedEvent::UserPrompt(UserPromptPayload {
                        text: text.value.clone(),
                        turn_ref: turn.clone(),
                    }),
                    self.content_sensitivity(&text),
                ));
            }
            "PreToolUse" => {
                let tool_ref = tool
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("tool_use_id"))?;
                let tool_kind = payload
                    .tool_name
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("tool_name"))?;
                let source_id = Some(Self::stable_id(&[
                    "PreToolUse",
                    &session,
                    turn.as_deref().unwrap_or("unknown"),
                    tool_ref,
                ]));
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), Some(tool_ref)),
                    source_id,
                    NormalizedEvent::ToolStarted(ToolStartedPayload {
                        tool_call_ref: tool_ref.to_owned(),
                        tool_kind: tool_kind.to_owned(),
                        summary: None,
                        turn_ref: turn.clone(),
                    }),
                    self.metadata_sensitivity(),
                ));
                if tool_kind == "Bash" {
                    if let Some(command) = self.command_text(payload.tool_input.as_ref())? {
                        events.push(self.pending(
                            hook,
                            &session,
                            (turn.as_deref(), Some(tool_ref)),
                            Some(Self::stable_id(&[
                                "command.executed",
                                &session,
                                turn.as_deref().unwrap_or("unknown"),
                                tool_ref,
                            ])),
                            NormalizedEvent::CommandExecuted(CommandExecutedPayload {
                                command_ref: tool_ref.to_owned(),
                                display: command.value.clone(),
                                working_directory: None,
                            }),
                            self.content_sensitivity(&command),
                        ));
                    }
                }
            }
            "PostToolUse" => {
                let tool_ref = tool
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("tool_use_id"))?;
                let tool_kind = payload
                    .tool_name
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("tool_name"))?;
                let retain_response = tool_kind == "Bash"
                    || self.file_tool_content_allowed(
                        payload.tool_input.as_ref(),
                        payload.tool_response.as_ref(),
                    );
                let response = if retain_response {
                    payload
                        .tool_response
                        .as_ref()
                        .map(|value| self.sanitize_json(value))
                        .transpose()?
                } else {
                    None
                };
                let sensitivity = response
                    .as_ref()
                    .map(|value| self.content_sensitivity(value))
                    .unwrap_or_else(|| self.metadata_sensitivity());
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), Some(tool_ref)),
                    Some(Self::stable_id(&[
                        "PostToolUse",
                        &session,
                        turn.as_deref().unwrap_or("unknown"),
                        tool_ref,
                    ])),
                    NormalizedEvent::ToolCompleted(ToolCompletedPayload {
                        tool_call_ref: tool_ref.to_owned(),
                        status: CompletionStatus::Unknown,
                        summary: response.as_ref().map(|value| value.value.clone()),
                        duration_ms: None,
                    }),
                    sensitivity.clone(),
                ));
                if tool_kind == "Bash" {
                    events.push(self.pending(
                        hook,
                        &session,
                        (turn.as_deref(), Some(tool_ref)),
                        Some(Self::stable_id(&[
                            "command.result",
                            &session,
                            turn.as_deref().unwrap_or("unknown"),
                            tool_ref,
                        ])),
                        NormalizedEvent::CommandResult(CommandResultPayload {
                            command_ref: tool_ref.to_owned(),
                            exit_code: None,
                            output: response.as_ref().map(|value| value.value.clone()),
                            duration_ms: None,
                        }),
                        sensitivity,
                    ));
                }
            }
            "PermissionRequest" => {
                let tool_kind = payload
                    .tool_name
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("tool_name"))?;
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), tool.as_deref()),
                    None,
                    NormalizedEvent::PermissionRequested(PermissionRequestedPayload {
                        tool_kind: tool_kind.to_owned(),
                        tool_call_ref: tool.clone(),
                        turn_ref: turn.clone(),
                        summary: None,
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "Stop" => {
                let text = payload
                    .last_assistant_message
                    .as_deref()
                    .ok_or(IntakeError::MissingRequired("last_assistant_message"))?;
                let text = self.sanitize(text)?;
                let turn_key = turn.as_deref().unwrap_or("unknown");
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    Some(Self::stable_id(&["agent.message", &session, turn_key])),
                    NormalizedEvent::AgentMessage(AgentMessagePayload {
                        text: text.value.clone(),
                        turn_ref: turn.clone(),
                        message_kind: MessageKind::Final,
                    }),
                    self.content_sensitivity(&text),
                ));
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    Some(Self::stable_id(&["turn.completed", &session, turn_key])),
                    NormalizedEvent::TurnCompleted(TurnCompletedPayload {
                        turn_ref: turn.clone(),
                        status: CompletionStatus::Unknown,
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "Interrupt" => {
                let source_id = turn
                    .as_deref()
                    .map(|turn| Self::stable_id(&["turn.interrupted", &session, turn]));
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    source_id,
                    NormalizedEvent::TurnCompleted(TurnCompletedPayload {
                        turn_ref: turn.clone(),
                        status: CompletionStatus::Cancelled,
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "SessionEnd" => {
                events.push(self.pending(
                    hook,
                    &session,
                    (None, None),
                    None,
                    NormalizedEvent::SessionStopped(SessionStoppedPayload {
                        reason: SessionStopReason::Unknown,
                        end_boundary_known: true,
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "PreCompact" | "PostCompact" => {
                let phase = if hook == "PreCompact" {
                    CompactionPhase::Before
                } else {
                    CompactionPhase::After
                };
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    None,
                    NormalizedEvent::ContextCompacted(ContextCompactedPayload {
                        phase,
                        turn_ref: turn.clone(),
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            "SubagentStart" | "SubagentStop" => {
                events.push(self.pending(
                    hook,
                    &session,
                    (turn.as_deref(), None),
                    None,
                    NormalizedEvent::CaptureGap(CaptureGapPayload {
                        reason: CaptureGapReason::Unsupported,
                        from: None,
                        to: None,
                        dropped_count: Some(1),
                    }),
                    self.metadata_sensitivity(),
                ));
            }
            _ => {
                events.push(self.pending(
                    "unknown",
                    &session,
                    (turn.as_deref(), None),
                    None,
                    NormalizedEvent::CaptureGap(CaptureGapPayload {
                        reason: CaptureGapReason::Unsupported,
                        from: None,
                        to: None,
                        dropped_count: Some(1),
                    }),
                    self.metadata_sensitivity(),
                ));
            }
        }
        Ok(events)
    }
}

fn collect_file_paths(value: &Value, paths: &mut Vec<String>, depth: usize) -> bool {
    if depth > 8 || paths.len() > 32 {
        return false;
    }
    match value {
        Value::Object(object) => {
            for (key, item) in object {
                if matches!(
                    key.as_str(),
                    "path"
                        | "file_path"
                        | "filePath"
                        | "old_path"
                        | "new_path"
                        | "target_path"
                        | "file"
                        | "filename"
                ) {
                    if let Some(path) = item.as_str() {
                        paths.push(path.to_owned());
                    } else {
                        return false;
                    }
                } else if key == "patch" {
                    if let Some(patch) = item.as_str() {
                        for line in patch.lines() {
                            for prefix in [
                                "*** Add File: ",
                                "*** Update File: ",
                                "*** Delete File: ",
                                "*** Move to: ",
                            ] {
                                if let Some(path) = line.strip_prefix(prefix) {
                                    paths.push(path.to_owned());
                                }
                            }
                        }
                    } else {
                        return false;
                    }
                } else {
                    if !collect_file_paths(item, paths, depth + 1) {
                        return false;
                    }
                }
                if paths.len() > 32 {
                    return false;
                }
            }
        }
        Value::Array(items) => {
            if items.len() > 32 {
                return false;
            }
            for item in items {
                if !collect_file_paths(item, paths, depth + 1) {
                    return false;
                }
            }
        }
        Value::String(text) => {
            for line in text.lines() {
                for prefix in [
                    "*** Add File: ",
                    "*** Update File: ",
                    "*** Delete File: ",
                    "*** Move to: ",
                ] {
                    if let Some(path) = line.strip_prefix(prefix) {
                        paths.push(path.to_owned());
                    }
                }
                if paths.len() > 32 {
                    return false;
                }
            }
        }
        _ => {}
    }
    true
}

impl From<SanitizeError> for IntakeError {
    fn from(_: SanitizeError) -> Self {
        Self::SanitizationFailed
    }
}
