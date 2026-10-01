use crate::{AssemblyError, AssemblyResult};
use mochi_capture::model as capture;
use mochi_domain as domain;
use mochi_persistence::{AssemblySourceKey, PersistedIngress};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const ASSEMBLY_VERSION: u16 = 1;
const UNSCOPED_TURN: &str = "__mochi_unscoped_turn__";

pub fn assembly_group_key(source: &AssemblySourceKey) -> String {
    let project_id = source.project_id.to_string();
    format!(
        "assembly-v1:{}",
        hash_parts(
            b"mochi-assembly-group-v1",
            &[
                &project_id,
                &source.provider,
                &source.adapter_version,
                &source.transport,
                surface_text(source.client_surface),
                &source.external_session_id,
            ]
        )
    )
}

pub fn session_id_for_source(source: &AssemblySourceKey) -> domain::SessionId {
    let group_key = assembly_group_key(source);
    domain::SessionId::parse(&deterministic_uuid("session", &[&group_key]).to_string())
        .expect("deterministic UUID is valid")
}

pub fn evidence_fingerprint(evidence: &[PersistedIngress]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"mochi-assembly-evidence-v1");
    for item in evidence {
        update_part(&mut digest, item.record.id.to_string().as_bytes());
        update_part(&mut digest, item.source_identity_key.as_bytes());
        digest.update(item.record.receive_sequence.to_be_bytes());
    }
    format!("evidence-v1:{:x}", digest.finalize())
}

pub fn assemble_session(
    source: &AssemblySourceKey,
    evidence: &[PersistedIngress],
    explicit_git_context: Option<&domain::GitContext>,
) -> AssemblyResult<domain::CodingSession> {
    if evidence.is_empty() {
        return Err(AssemblyError::InvalidEvidenceRelationship);
    }
    validate_evidence_group(source, evidence)?;
    let session_id = session_id_for_source(source);
    let turn_plan = build_turn_plan(session_id, evidence)?;
    let events = build_events(source, session_id, evidence, &turn_plan)?;
    let turns = build_turns(session_id, evidence, &events, &turn_plan)?;
    let tools = build_tools(session_id, evidence, &turn_plan)?;
    let commands = build_commands(session_id, evidence, &turn_plan, &tools)?;
    let git_context = explicit_git_context
        .cloned()
        .unwrap_or(domain::GitContext::Unavailable {
            reason: domain::GitUnavailableReason::NotCaptured,
        });
    git_context
        .validate()
        .map_err(|_| AssemblyError::InvalidEvidenceRelationship)?;
    let file_changes = derive_git_file_changes(session_id, &git_context);
    let capabilities = capture_capabilities(source, evidence, &git_context)?;
    let (status, ended_at, end_known) = session_lifecycle(evidence)?;
    let started_at = evidence
        .iter()
        .find(|item| {
            matches!(
                item.record.event,
                capture::NormalizedEvent::SessionStarted(_)
            )
        })
        .unwrap_or(&evidence[0])
        .record
        .received_at
        .as_str();
    let started_at = parse_timestamp(started_at)?;
    let completeness = capture_completeness(
        evidence,
        &capabilities,
        &tools,
        &commands,
        &file_changes,
        &git_context,
        end_known,
    );
    let data = domain::CodingSessionData {
        schema_version: domain::CODING_SESSION_SCHEMA_VERSION,
        id: session_id,
        project_id: source.project_id,
        source: domain_source(source),
        started_at,
        ended_at,
        status,
        turns,
        events,
        tool_executions: tools,
        command_executions: commands,
        file_changes,
        git_context,
        capture_capabilities: capabilities,
        capture_completeness: completeness,
    };
    domain::CodingSession::new(data).map_err(|_| AssemblyError::DomainValidation)
}

fn validate_evidence_group(
    source: &AssemblySourceKey,
    evidence: &[PersistedIngress],
) -> AssemblyResult<()> {
    let mut ingress_ids = HashSet::new();
    let mut receive_sequences = HashSet::new();
    let mut previous = None;
    for item in evidence {
        let record = &item.record;
        if record.schema_version != capture::INGRESS_SCHEMA_VERSION {
            return Err(AssemblyError::UnsupportedIngressSchema);
        }
        if record.project_id != source.project_id.as_uuid()
            || record.source.provider != source.provider
            || record.source.adapter_version != source.adapter_version
            || record.source.transport != source.transport
            || record.source.client_surface != source.client_surface
            || record.external_session_id.as_deref() != Some(&source.external_session_id)
            || !ingress_ids.insert(record.id)
            || !receive_sequences.insert(record.receive_sequence)
            || previous.is_some_and(|value| value > record.receive_sequence)
        {
            return Err(AssemblyError::InvalidEvidenceRelationship);
        }
        previous = Some(record.receive_sequence);
        parse_timestamp(&record.received_at)?;
        if let Some(timestamp) = &record.source_timestamp {
            parse_timestamp(timestamp)?;
        }
    }
    Ok(())
}

struct TurnPlan {
    ids: HashMap<String, domain::TurnId>,
    order: Vec<String>,
    use_unscoped: bool,
}

fn build_turn_plan(
    session_id: domain::SessionId,
    evidence: &[PersistedIngress],
) -> AssemblyResult<TurnPlan> {
    let mut refs = Vec::new();
    for item in evidence {
        if let Some(value) = resolved_turn_ref(&item.record)?
            && !refs.contains(&value)
        {
            refs.push(value);
        }
    }
    let use_unscoped = refs.is_empty()
        && evidence
            .iter()
            .any(|item| is_turn_scoped(&item.record.event));
    if use_unscoped {
        refs.push(UNSCOPED_TURN.to_owned());
    }
    let ids = refs
        .iter()
        .map(|value| {
            (
                value.clone(),
                domain::TurnId::parse(
                    &deterministic_uuid("turn", &[&session_id.to_string(), value]).to_string(),
                )
                .expect("deterministic UUID is valid"),
            )
        })
        .collect();
    Ok(TurnPlan {
        ids,
        order: refs,
        use_unscoped,
    })
}

fn event_turn_id(
    record: &capture::SpoolIngressRecord,
    plan: &TurnPlan,
) -> AssemblyResult<Option<domain::TurnId>> {
    let key = resolved_turn_ref(record)?.or_else(|| {
        (plan.use_unscoped && is_turn_scoped(&record.event)).then(|| UNSCOPED_TURN.to_owned())
    });
    key.map(|key| {
        plan.ids
            .get(&key)
            .copied()
            .ok_or(AssemblyError::InvalidEvidenceRelationship)
    })
    .transpose()
}

fn resolved_turn_ref(record: &capture::SpoolIngressRecord) -> AssemblyResult<Option<String>> {
    let payload_ref = match &record.event {
        capture::NormalizedEvent::UserPrompt(value) => value.turn_ref.as_deref(),
        capture::NormalizedEvent::AgentMessage(value) => value.turn_ref.as_deref(),
        capture::NormalizedEvent::ToolStarted(value) => value.turn_ref.as_deref(),
        capture::NormalizedEvent::PermissionRequested(value) => value.turn_ref.as_deref(),
        capture::NormalizedEvent::TurnCompleted(value) => value.turn_ref.as_deref(),
        capture::NormalizedEvent::ContextCompacted(value) => value.turn_ref.as_deref(),
        _ => None,
    };
    if let (Some(envelope), Some(payload)) = (record.external_turn_id.as_deref(), payload_ref)
        && envelope != payload
    {
        return Err(AssemblyError::InvalidEvidenceRelationship);
    }
    Ok(record
        .external_turn_id
        .as_deref()
        .or(payload_ref)
        .map(str::to_owned))
}

fn is_turn_scoped(event: &capture::NormalizedEvent) -> bool {
    matches!(
        event,
        capture::NormalizedEvent::UserPrompt(_)
            | capture::NormalizedEvent::AgentMessage(_)
            | capture::NormalizedEvent::ToolStarted(_)
            | capture::NormalizedEvent::ToolCompleted(_)
            | capture::NormalizedEvent::PermissionRequested(_)
            | capture::NormalizedEvent::CommandExecuted(_)
            | capture::NormalizedEvent::CommandResult(_)
            | capture::NormalizedEvent::TurnCompleted(_)
            | capture::NormalizedEvent::ContextCompacted(_)
    )
}

fn build_events(
    source: &AssemblySourceKey,
    session_id: domain::SessionId,
    evidence: &[PersistedIngress],
    plan: &TurnPlan,
) -> AssemblyResult<Vec<domain::SessionEvent>> {
    evidence
        .iter()
        .map(|item| {
            let record = &item.record;
            Ok(domain::SessionEvent {
                schema_version: domain::SESSION_EVENT_SCHEMA_VERSION,
                id: domain::EventId::parse(&record.id.to_string())
                    .map_err(|_| AssemblyError::InvalidEvidenceRelationship)?,
                session_id,
                project_id: source.project_id,
                turn_id: event_turn_id(record, plan)?,
                source: domain_source(source),
                provenance: domain::EventProvenance {
                    source_event_id: record.source_event_id.clone(),
                    source_sequence: record.source_sequence,
                    source_event_type: Some(record.source_event_type.clone()),
                    occurred_at: record
                        .source_timestamp
                        .as_deref()
                        .map(parse_timestamp)
                        .transpose()?,
                },
                received_at: parse_timestamp(&record.received_at)?,
                sequence: record.receive_sequence,
                origin: map_origin(record.origin),
                event: map_event(&record.event)?,
                sensitivity: domain::Sensitivity {
                    classification: match record.sensitivity.classification {
                        capture::SensitivityClassification::Sanitized => {
                            domain::SensitivityClassification::Sanitized
                        }
                        capture::SensitivityClassification::MetadataOnly => {
                            domain::SensitivityClassification::MetadataOnly
                        }
                    },
                    redaction_count: record.sensitivity.redaction_count,
                    rules_version: record.sensitivity.rules_version.clone(),
                    policy_revision: record.sensitivity.policy_revision,
                    truncated: record.sensitivity.truncated,
                },
            })
        })
        .collect()
}

fn build_turns(
    session_id: domain::SessionId,
    evidence: &[PersistedIngress],
    events: &[domain::SessionEvent],
    plan: &TurnPlan,
) -> AssemblyResult<Vec<domain::SessionTurn>> {
    let mut turns = Vec::new();
    for key in &plan.order {
        let id = plan.ids[key];
        let mut event_ids = Vec::new();
        let mut started_at = None;
        let mut prompt = None;
        let mut response = None;
        let mut completion: Option<(domain::UtcTimestamp, capture::CompletionStatus)> = None;
        for (item, event) in evidence.iter().zip(events) {
            if event.turn_id != Some(id) {
                continue;
            }
            event_ids.push(event.id);
            started_at.get_or_insert_with(|| event.received_at.clone());
            match &item.record.event {
                capture::NormalizedEvent::UserPrompt(value) => {
                    set_once(&mut prompt, value.text.clone())?;
                }
                capture::NormalizedEvent::AgentMessage(value)
                    if value.message_kind == capture::MessageKind::Final =>
                {
                    set_once(&mut response, value.text.clone())?;
                }
                capture::NormalizedEvent::TurnCompleted(value) => {
                    if completion.is_some() {
                        return Err(AssemblyError::AmbiguousAssociation);
                    }
                    completion = Some((event.received_at.clone(), value.status));
                }
                _ => {}
            }
        }
        let (ended_at, status) = match completion {
            Some((time, capture::CompletionStatus::Succeeded)) => {
                (Some(time), domain::TurnStatus::Completed)
            }
            Some((time, capture::CompletionStatus::Failed)) => {
                (Some(time), domain::TurnStatus::Failed)
            }
            Some((time, capture::CompletionStatus::Cancelled)) => {
                (Some(time), domain::TurnStatus::Interrupted)
            }
            Some((time, capture::CompletionStatus::Unknown)) => {
                (Some(time), domain::TurnStatus::Incomplete)
            }
            None => (None, domain::TurnStatus::Incomplete),
        };
        turns.push(domain::SessionTurn {
            id,
            session_id,
            source_turn_id: (key != UNSCOPED_TURN).then(|| key.clone()),
            started_at,
            ended_at,
            user_prompt: prompt,
            agent_final_response: response,
            event_ids,
            status,
        });
    }
    Ok(turns)
}

#[derive(Default)]
struct ToolAccumulator<'a> {
    start: Option<(
        &'a capture::SpoolIngressRecord,
        &'a capture::ToolStartedPayload,
    )>,
    completion: Option<(
        &'a capture::SpoolIngressRecord,
        &'a capture::ToolCompletedPayload,
    )>,
}

fn build_tools(
    session_id: domain::SessionId,
    evidence: &[PersistedIngress],
    plan: &TurnPlan,
) -> AssemblyResult<Vec<domain::ToolExecution>> {
    let mut order = Vec::new();
    let mut accumulators: HashMap<String, ToolAccumulator<'_>> = HashMap::new();
    for item in evidence {
        let (reference, is_start) = match &item.record.event {
            capture::NormalizedEvent::ToolStarted(value) => (value.tool_call_ref.as_str(), true),
            capture::NormalizedEvent::ToolCompleted(value) => (value.tool_call_ref.as_str(), false),
            _ => continue,
        };
        validate_tool_ref(&item.record, reference)?;
        if !accumulators.contains_key(reference) {
            order.push(reference.to_owned());
        }
        let accumulator = accumulators.entry(reference.to_owned()).or_default();
        match (&item.record.event, is_start) {
            (capture::NormalizedEvent::ToolStarted(value), true) => {
                if accumulator.start.replace((&item.record, value)).is_some() {
                    return Err(AssemblyError::AmbiguousAssociation);
                }
            }
            (capture::NormalizedEvent::ToolCompleted(value), false) => {
                if accumulator
                    .completion
                    .replace((&item.record, value))
                    .is_some()
                {
                    return Err(AssemblyError::AmbiguousAssociation);
                }
            }
            _ => unreachable!(),
        }
    }
    let interrupted_turns = interrupted_turns(evidence, plan)?;
    let mut tools = Vec::new();
    for reference in order {
        let accumulator = &accumulators[&reference];
        let Some((start_record, start)) = accumulator.start else {
            continue;
        };
        let turn_id = event_turn_id(start_record, plan)?;
        let (completed_at, status, output_summary) = match accumulator.completion {
            Some((record, completed)) => (
                Some(parse_timestamp(&record.received_at)?),
                map_tool_status(completed.status),
                completed.summary.clone(),
            ),
            None if turn_id.is_some_and(|id| interrupted_turns.contains(&id)) => {
                (None, domain::ToolExecutionStatus::Interrupted, None)
            }
            None => (None, domain::ToolExecutionStatus::Incomplete, None),
        };
        tools.push(domain::ToolExecution {
            id: domain::ToolExecutionId::parse(
                &deterministic_uuid("tool", &[&session_id.to_string(), &reference]).to_string(),
            )
            .expect("deterministic UUID is valid"),
            session_id,
            turn_id,
            source_tool_use_id: Some(reference),
            tool_name: start.tool_kind.clone(),
            started_at: Some(parse_timestamp(&start_record.received_at)?),
            completed_at,
            status,
            input_summary: start.summary.clone(),
            output_summary,
            related_file_paths: Vec::new(),
        });
    }
    Ok(tools)
}

#[derive(Default)]
struct CommandAccumulator<'a> {
    start: Option<(
        &'a capture::SpoolIngressRecord,
        &'a capture::CommandExecutedPayload,
    )>,
    result: Option<(
        &'a capture::SpoolIngressRecord,
        &'a capture::CommandResultPayload,
    )>,
}

fn build_commands(
    session_id: domain::SessionId,
    evidence: &[PersistedIngress],
    plan: &TurnPlan,
    tools: &[domain::ToolExecution],
) -> AssemblyResult<Vec<domain::CommandExecution>> {
    let mut order = Vec::new();
    let mut accumulators: HashMap<String, CommandAccumulator<'_>> = HashMap::new();
    for item in evidence {
        let reference = match &item.record.event {
            capture::NormalizedEvent::CommandExecuted(value) => value.command_ref.as_str(),
            capture::NormalizedEvent::CommandResult(value) => value.command_ref.as_str(),
            _ => continue,
        };
        validate_tool_ref(&item.record, reference)?;
        if !accumulators.contains_key(reference) {
            order.push(reference.to_owned());
        }
        let accumulator = accumulators.entry(reference.to_owned()).or_default();
        match &item.record.event {
            capture::NormalizedEvent::CommandExecuted(value) => {
                if accumulator.start.replace((&item.record, value)).is_some() {
                    return Err(AssemblyError::AmbiguousAssociation);
                }
            }
            capture::NormalizedEvent::CommandResult(value) => {
                if accumulator.result.replace((&item.record, value)).is_some() {
                    return Err(AssemblyError::AmbiguousAssociation);
                }
            }
            _ => unreachable!(),
        }
    }
    let interrupted_turns = interrupted_turns(evidence, plan)?;
    let mut commands = Vec::new();
    for reference in order {
        let accumulator = &accumulators[&reference];
        let Some((start_record, start)) = accumulator.start else {
            continue;
        };
        let turn_id = event_turn_id(start_record, plan)?;
        let tool = tools
            .iter()
            .find(|tool| tool.source_tool_use_id.as_deref() == Some(&reference));
        let (completed_at, status, exit_code, output_summary) = match accumulator.result {
            Some((record, result)) => {
                let status = match result.exit_code {
                    Some(0) => domain::CommandExecutionStatus::Completed,
                    Some(_) => domain::CommandExecutionStatus::Failed,
                    None if tool.is_some_and(|value| {
                        value.status == domain::ToolExecutionStatus::Interrupted
                    }) =>
                    {
                        domain::CommandExecutionStatus::Interrupted
                    }
                    None => domain::CommandExecutionStatus::Unknown,
                };
                (
                    Some(parse_timestamp(&record.received_at)?),
                    status,
                    result.exit_code,
                    result.output.clone(),
                )
            }
            None if turn_id.is_some_and(|id| interrupted_turns.contains(&id)) => (
                None,
                domain::CommandExecutionStatus::Interrupted,
                None,
                None,
            ),
            None => (None, domain::CommandExecutionStatus::Incomplete, None, None),
        };
        commands.push(domain::CommandExecution {
            id: domain::CommandExecutionId::parse(
                &deterministic_uuid("command", &[&session_id.to_string(), &reference]).to_string(),
            )
            .expect("deterministic UUID is valid"),
            session_id,
            turn_id,
            tool_execution_id: tool.map(|value| value.id),
            command: start.display.clone(),
            working_directory: start.working_directory.clone(),
            started_at: Some(parse_timestamp(&start_record.received_at)?),
            completed_at,
            status,
            exit_code,
            output_summary,
        });
    }
    Ok(commands)
}

fn interrupted_turns(
    evidence: &[PersistedIngress],
    plan: &TurnPlan,
) -> AssemblyResult<HashSet<domain::TurnId>> {
    let mut result = HashSet::new();
    for item in evidence {
        if let capture::NormalizedEvent::TurnCompleted(value) = &item.record.event
            && value.status == capture::CompletionStatus::Cancelled
            && let Some(id) = event_turn_id(&item.record, plan)?
        {
            result.insert(id);
        }
    }
    Ok(result)
}

fn session_lifecycle(
    evidence: &[PersistedIngress],
) -> AssemblyResult<(domain::SessionStatus, Option<domain::UtcTimestamp>, bool)> {
    let stops = evidence
        .iter()
        .filter_map(|item| match &item.record.event {
            capture::NormalizedEvent::SessionStopped(value) => Some((item, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if stops.len() > 1 {
        return Err(AssemblyError::AmbiguousAssociation);
    }
    let Some((item, stop)) = stops.first().copied() else {
        return Ok((domain::SessionStatus::Incomplete, None, false));
    };
    if !stop.end_boundary_known {
        return Ok((domain::SessionStatus::Incomplete, None, false));
    }
    let status = if stop.reason == capture::SessionStopReason::Interrupted {
        domain::SessionStatus::Interrupted
    } else {
        domain::SessionStatus::Completed
    };
    Ok((
        status,
        Some(parse_timestamp(&item.record.received_at)?),
        true,
    ))
}

fn capture_capabilities(
    source: &AssemblySourceKey,
    evidence: &[PersistedIngress],
    git: &domain::GitContext,
) -> AssemblyResult<domain::CaptureCapabilities> {
    let reports = evidence
        .iter()
        .filter_map(|item| match &item.record.event {
            capture::NormalizedEvent::SessionStarted(value) => Some(&value.capabilities),
            _ => None,
        })
        .collect::<Vec<_>>();
    if reports.windows(2).any(|pair| pair[0] != pair[1]) {
        return Err(AssemblyError::AmbiguousAssociation);
    }
    let mut capabilities = if let Some(report) = reports.first() {
        domain::CaptureCapabilities {
            session_lifecycle: map_capability(report.session_end),
            user_prompt: map_capability(report.prompts),
            agent_response: map_capability(report.agent_messages),
            tool_activity: map_capability(report.tool_lifecycle),
            commands: map_capability(report.commands),
            file_activity: map_capability(report.file_events),
            permissions: map_capability(report.permissions),
            interrupts: map_capability(report.interrupts),
            git_context: domain::CapabilitySupport::Unknown,
        }
    } else {
        fallback_capabilities(source.client_surface)
    };
    capabilities.git_context = match git {
        domain::GitContext::Available { .. } => domain::CapabilitySupport::Supported,
        domain::GitContext::Unavailable {
            reason: domain::GitUnavailableReason::NotRepository,
        } => domain::CapabilitySupport::Unsupported,
        domain::GitContext::Unavailable { .. } => domain::CapabilitySupport::Unknown,
    };
    Ok(capabilities)
}

fn fallback_capabilities(surface: capture::ClientSurface) -> domain::CaptureCapabilities {
    let known = matches!(surface, capture::ClientSurface::Cli);
    domain::CaptureCapabilities {
        session_lifecycle: domain::CapabilitySupport::Supported,
        user_prompt: if known {
            domain::CapabilitySupport::Supported
        } else {
            domain::CapabilitySupport::Unknown
        },
        agent_response: domain::CapabilitySupport::Supported,
        tool_activity: domain::CapabilitySupport::Supported,
        commands: domain::CapabilitySupport::Supported,
        file_activity: domain::CapabilitySupport::Unknown,
        permissions: domain::CapabilitySupport::Supported,
        interrupts: if known {
            domain::CapabilitySupport::Supported
        } else {
            domain::CapabilitySupport::Unknown
        },
        git_context: domain::CapabilitySupport::Unknown,
    }
}

fn capture_completeness(
    evidence: &[PersistedIngress],
    capabilities: &domain::CaptureCapabilities,
    tools: &[domain::ToolExecution],
    commands: &[domain::CommandExecution],
    files: &[domain::FileChange],
    git: &domain::GitContext,
    end_known: bool,
) -> domain::CaptureCompleteness {
    let start = evidence.iter().find_map(|item| match &item.record.event {
        capture::NormalizedEvent::SessionStarted(value) => Some(value.start_boundary_known),
        _ => None,
    });
    let has_prompt = evidence
        .iter()
        .any(|item| matches!(item.record.event, capture::NormalizedEvent::UserPrompt(_)));
    let has_final = evidence.iter().any(|item| {
        matches!(
            &item.record.event,
            capture::NormalizedEvent::AgentMessage(value)
                if value.message_kind == capture::MessageKind::Final
        )
    });
    let tool_events = evidence
        .iter()
        .filter(|item| {
            matches!(
                item.record.event,
                capture::NormalizedEvent::ToolStarted(_)
                    | capture::NormalizedEvent::ToolCompleted(_)
            )
        })
        .count();
    let command_events = evidence
        .iter()
        .filter(|item| {
            matches!(
                item.record.event,
                capture::NormalizedEvent::CommandExecuted(_)
                    | capture::NormalizedEvent::CommandResult(_)
            )
        })
        .count();
    let interrupted = evidence.iter().any(|item| match &item.record.event {
        capture::NormalizedEvent::TurnCompleted(value) => {
            value.status == capture::CompletionStatus::Cancelled
        }
        capture::NormalizedEvent::ToolCompleted(value) => {
            value.status == capture::CompletionStatus::Cancelled
        }
        capture::NormalizedEvent::SessionStopped(value) => {
            value.reason == capture::SessionStopReason::Interrupted
        }
        _ => false,
    });
    let has_gap = evidence.iter().any(|item| {
        matches!(item.record.event, capture::NormalizedEvent::CaptureGap(_))
            || item.record.sensitivity.truncated
    });
    let mut result = domain::CaptureCompleteness {
        session_lifecycle: match (start, end_known, capabilities.session_lifecycle) {
            (Some(true), true, _) => domain::EvidenceCompleteness::Complete,
            (None, false, domain::CapabilitySupport::Supported) => {
                domain::EvidenceCompleteness::Missing
            }
            _ => domain::EvidenceCompleteness::Partial,
        },
        prompt: evidence_dimension(has_prompt, capabilities.user_prompt, true),
        agent_response: evidence_dimension(has_final, capabilities.agent_response, true),
        tools: activity_dimension(
            tool_events,
            tools.len().saturating_mul(2),
            capabilities.tool_activity,
        ),
        commands: activity_dimension(
            command_events,
            commands.len().saturating_mul(2),
            capabilities.commands,
        ),
        files: if !files.is_empty() {
            domain::EvidenceCompleteness::Partial
        } else {
            absence_dimension(capabilities.file_activity)
        },
        git: match git {
            domain::GitContext::Available { before, after } => {
                if before.truncated
                    || after.as_ref().is_none_or(|value| value.truncated)
                    || after.is_none()
                    || after.as_ref().is_some_and(|value| {
                        before.branch != value.branch || before.detached != value.detached
                    })
                {
                    domain::EvidenceCompleteness::Partial
                } else {
                    domain::EvidenceCompleteness::Complete
                }
            }
            domain::GitContext::Unavailable {
                reason: domain::GitUnavailableReason::NotRepository,
            } => domain::EvidenceCompleteness::NotApplicable,
            domain::GitContext::Unavailable { .. } => domain::EvidenceCompleteness::Unknown,
        },
        interrupts: if interrupted {
            domain::EvidenceCompleteness::Complete
        } else {
            absence_dimension(capabilities.interrupts)
        },
    };
    if has_gap {
        degrade_complete(&mut result.session_lifecycle);
        degrade_complete(&mut result.prompt);
        degrade_complete(&mut result.agent_response);
        degrade_complete(&mut result.tools);
        degrade_complete(&mut result.commands);
        degrade_complete(&mut result.files);
        degrade_complete(&mut result.interrupts);
    }
    result
}

fn evidence_dimension(
    present: bool,
    capability: domain::CapabilitySupport,
    expected: bool,
) -> domain::EvidenceCompleteness {
    if present {
        return domain::EvidenceCompleteness::Complete;
    }
    match capability {
        domain::CapabilitySupport::Supported if expected => domain::EvidenceCompleteness::Missing,
        domain::CapabilitySupport::Partial => domain::EvidenceCompleteness::Partial,
        domain::CapabilitySupport::Unsupported => domain::EvidenceCompleteness::NotApplicable,
        _ => domain::EvidenceCompleteness::Unknown,
    }
}

fn activity_dimension(
    event_count: usize,
    paired_capacity: usize,
    capability: domain::CapabilitySupport,
) -> domain::EvidenceCompleteness {
    if event_count == 0 {
        return absence_dimension(capability);
    }
    if event_count == paired_capacity {
        domain::EvidenceCompleteness::Complete
    } else {
        domain::EvidenceCompleteness::Partial
    }
}

fn absence_dimension(capability: domain::CapabilitySupport) -> domain::EvidenceCompleteness {
    match capability {
        domain::CapabilitySupport::Supported | domain::CapabilitySupport::Unsupported => {
            domain::EvidenceCompleteness::NotApplicable
        }
        domain::CapabilitySupport::Partial => domain::EvidenceCompleteness::Partial,
        domain::CapabilitySupport::Unknown => domain::EvidenceCompleteness::Unknown,
    }
}

fn degrade_complete(value: &mut domain::EvidenceCompleteness) {
    if *value == domain::EvidenceCompleteness::Complete {
        *value = domain::EvidenceCompleteness::Partial;
    }
}

fn derive_git_file_changes(
    session_id: domain::SessionId,
    context: &domain::GitContext,
) -> Vec<domain::FileChange> {
    let domain::GitContext::Available {
        before,
        after: Some(after),
    } = context
    else {
        return Vec::new();
    };
    if before.branch != after.branch || before.detached != after.detached {
        return Vec::new();
    }
    let before_by_path = before
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<HashMap<_, _>>();
    after
        .files
        .iter()
        .filter_map(|file| {
            let unchanged = before_by_path
                .get(file.path.as_str())
                .is_some_and(|value| *value == file);
            if unchanged {
                return None;
            }
            let change_type = file.change_type;
            Some(domain::FileChange {
                id: domain::FileChangeId::parse(
                    &deterministic_uuid(
                        "file",
                        &[
                            &session_id.to_string(),
                            &file.path,
                            file.previous_path.as_deref().unwrap_or(""),
                            file_change_text(change_type),
                        ],
                    )
                    .to_string(),
                )
                .expect("deterministic UUID is valid"),
                path: file.path.clone(),
                previous_path: file.previous_path.clone(),
                change_type,
                source: domain::FileChangeSource::GitComparison,
                observed_during_session: true,
                attribution: domain::FileAttribution::Ambiguous,
            })
        })
        .collect()
}

fn set_once<T>(target: &mut Option<T>, value: T) -> AssemblyResult<()> {
    if target.is_some() {
        Err(AssemblyError::AmbiguousAssociation)
    } else {
        *target = Some(value);
        Ok(())
    }
}

fn validate_tool_ref(
    record: &capture::SpoolIngressRecord,
    payload_ref: &str,
) -> AssemblyResult<()> {
    if payload_ref.is_empty()
        || record
            .external_tool_use_id
            .as_deref()
            .is_some_and(|value| value != payload_ref)
    {
        return Err(AssemblyError::InvalidEvidenceRelationship);
    }
    Ok(())
}

fn map_tool_status(value: capture::CompletionStatus) -> domain::ToolExecutionStatus {
    match value {
        capture::CompletionStatus::Succeeded => domain::ToolExecutionStatus::Completed,
        capture::CompletionStatus::Failed => domain::ToolExecutionStatus::Failed,
        capture::CompletionStatus::Cancelled => domain::ToolExecutionStatus::Interrupted,
        capture::CompletionStatus::Unknown => domain::ToolExecutionStatus::Incomplete,
    }
}

fn map_capability(value: capture::CapabilitySupport) -> domain::CapabilitySupport {
    match value {
        capture::CapabilitySupport::Supported => domain::CapabilitySupport::Supported,
        capture::CapabilitySupport::Unsupported => domain::CapabilitySupport::Unsupported,
        capture::CapabilitySupport::Unknown => domain::CapabilitySupport::Unknown,
    }
}

fn domain_source(source: &AssemblySourceKey) -> domain::SourceDescriptor {
    domain::SourceDescriptor {
        provider: source.provider.clone(),
        client_surface: match source.client_surface {
            capture::ClientSurface::Cli => domain::ClientSurface::Cli,
            capture::ClientSurface::Desktop => domain::ClientSurface::Desktop,
            capture::ClientSurface::Unknown => domain::ClientSurface::Unknown,
        },
        client_version: None,
        adapter_version: source.adapter_version.clone(),
        transport: source.transport.clone(),
    }
}

fn map_origin(value: capture::EventOrigin) -> domain::EventOrigin {
    match value {
        capture::EventOrigin::Provider => domain::EventOrigin::Provider,
        capture::EventOrigin::Git => domain::EventOrigin::Git,
        capture::EventOrigin::Mochi => domain::EventOrigin::Mochi,
    }
}

fn map_event(value: &capture::NormalizedEvent) -> AssemblyResult<domain::SessionEventData> {
    Ok(match value {
        capture::NormalizedEvent::SessionStarted(payload) => {
            domain::SessionEventData::SessionStarted(domain::SessionStartedPayload {
                reason: match payload.reason {
                    capture::SessionStartReason::Startup => domain::SessionStartReason::Startup,
                    capture::SessionStartReason::Resume => domain::SessionStartReason::Resume,
                    capture::SessionStartReason::Observed => domain::SessionStartReason::Observed,
                },
                capabilities: domain::SessionSourceCapabilities {
                    identity: map_source_capability(payload.capabilities.identity),
                    project_association: map_source_capability(
                        payload.capabilities.project_association,
                    ),
                    activity: map_source_capability(payload.capabilities.activity),
                    prompts: map_source_capability(payload.capabilities.prompts),
                    agent_messages: map_source_capability(payload.capabilities.agent_messages),
                    tool_lifecycle: map_source_capability(payload.capabilities.tool_lifecycle),
                    permissions: map_source_capability(payload.capabilities.permissions),
                    interrupts: map_source_capability(payload.capabilities.interrupts),
                    commands: map_source_capability(payload.capabilities.commands),
                    file_events: map_source_capability(payload.capabilities.file_events),
                    session_end: map_source_capability(payload.capabilities.session_end),
                },
                start_boundary_known: payload.start_boundary_known,
            })
        }
        capture::NormalizedEvent::UserPrompt(payload) => {
            domain::SessionEventData::UserPrompt(domain::UserPromptPayload {
                text: payload.text.clone(),
            })
        }
        capture::NormalizedEvent::AgentMessage(payload) => {
            domain::SessionEventData::AgentMessage(domain::AgentMessagePayload {
                text: payload.text.clone(),
                message_kind: match payload.message_kind {
                    capture::MessageKind::Progress => domain::MessageKind::Progress,
                    capture::MessageKind::Final => domain::MessageKind::Final,
                    capture::MessageKind::Unknown => domain::MessageKind::Unknown,
                },
            })
        }
        capture::NormalizedEvent::ToolStarted(payload) => {
            domain::SessionEventData::ToolStarted(domain::ToolStartedPayload {
                tool_call_ref: payload.tool_call_ref.clone(),
                tool_kind: payload.tool_kind.clone(),
                summary: payload.summary.clone(),
            })
        }
        capture::NormalizedEvent::ToolCompleted(payload) => {
            domain::SessionEventData::ToolCompleted(domain::ToolCompletedPayload {
                tool_call_ref: payload.tool_call_ref.clone(),
                status: map_completion(payload.status),
                summary: payload.summary.clone(),
                duration_ms: payload.duration_ms,
            })
        }
        capture::NormalizedEvent::PermissionRequested(payload) => {
            domain::SessionEventData::PermissionRequested(domain::PermissionRequestedPayload {
                tool_kind: payload.tool_kind.clone(),
                tool_call_ref: payload.tool_call_ref.clone(),
                summary: payload.summary.clone(),
            })
        }
        capture::NormalizedEvent::CommandExecuted(payload) => {
            domain::SessionEventData::CommandExecuted(domain::CommandExecutedPayload {
                command_ref: payload.command_ref.clone(),
                display: payload.display.clone(),
                working_directory: payload.working_directory.clone(),
            })
        }
        capture::NormalizedEvent::CommandResult(payload) => {
            domain::SessionEventData::CommandResult(domain::CommandResultPayload {
                command_ref: payload.command_ref.clone(),
                exit_code: payload.exit_code,
                output: payload.output.clone(),
                duration_ms: payload.duration_ms,
            })
        }
        capture::NormalizedEvent::TurnCompleted(payload) => {
            domain::SessionEventData::TurnCompleted(domain::TurnCompletedPayload {
                status: map_completion(payload.status),
            })
        }
        capture::NormalizedEvent::SessionStopped(payload) => {
            domain::SessionEventData::SessionStopped(domain::SessionStoppedPayload {
                reason: match payload.reason {
                    capture::SessionStopReason::Completed => domain::SessionStopReason::Completed,
                    capture::SessionStopReason::UserFinalized => {
                        domain::SessionStopReason::UserFinalized
                    }
                    capture::SessionStopReason::Interrupted => {
                        domain::SessionStopReason::Interrupted
                    }
                    capture::SessionStopReason::IdleConfirmed => {
                        domain::SessionStopReason::IdleConfirmed
                    }
                    capture::SessionStopReason::Unknown => domain::SessionStopReason::Unknown,
                },
                end_boundary_known: payload.end_boundary_known,
            })
        }
        capture::NormalizedEvent::ContextCompacted(payload) => {
            domain::SessionEventData::ContextCompacted(domain::ContextCompactedPayload {
                phase: match payload.phase {
                    capture::CompactionPhase::Before => domain::CompactionPhase::Before,
                    capture::CompactionPhase::After => domain::CompactionPhase::After,
                },
            })
        }
        capture::NormalizedEvent::CaptureGap(payload) => {
            domain::SessionEventData::CaptureGap(domain::CaptureGapPayload {
                reason: match payload.reason {
                    capture::CaptureGapReason::MissingStart => {
                        domain::CaptureGapReason::MissingStart
                    }
                    capture::CaptureGapReason::MissingEnd => domain::CaptureGapReason::MissingEnd,
                    capture::CaptureGapReason::Paused => domain::CaptureGapReason::Paused,
                    capture::CaptureGapReason::Overflow => domain::CaptureGapReason::Overflow,
                    capture::CaptureGapReason::Corrupt => domain::CaptureGapReason::Corrupt,
                    capture::CaptureGapReason::Unsupported => domain::CaptureGapReason::Unsupported,
                    capture::CaptureGapReason::Restart => domain::CaptureGapReason::Restart,
                    capture::CaptureGapReason::AmbiguousAttribution => {
                        domain::CaptureGapReason::AmbiguousAttribution
                    }
                    capture::CaptureGapReason::InvalidInput => {
                        domain::CaptureGapReason::InvalidInput
                    }
                    capture::CaptureGapReason::SanitizationFailed => {
                        domain::CaptureGapReason::SanitizationFailed
                    }
                },
                from: payload.from.as_deref().map(parse_timestamp).transpose()?,
                to: payload.to.as_deref().map(parse_timestamp).transpose()?,
                dropped_count: payload.dropped_count,
            })
        }
    })
}

fn map_source_capability(value: capture::CapabilitySupport) -> domain::SourceCapabilitySupport {
    match value {
        capture::CapabilitySupport::Supported => domain::SourceCapabilitySupport::Supported,
        capture::CapabilitySupport::Unsupported => domain::SourceCapabilitySupport::Unsupported,
        capture::CapabilitySupport::Unknown => domain::SourceCapabilitySupport::Unknown,
    }
}

fn map_completion(value: capture::CompletionStatus) -> domain::CompletionStatus {
    match value {
        capture::CompletionStatus::Succeeded => domain::CompletionStatus::Succeeded,
        capture::CompletionStatus::Failed => domain::CompletionStatus::Failed,
        capture::CompletionStatus::Cancelled => domain::CompletionStatus::Cancelled,
        capture::CompletionStatus::Unknown => domain::CompletionStatus::Unknown,
    }
}

fn parse_timestamp(value: &str) -> AssemblyResult<domain::UtcTimestamp> {
    domain::UtcTimestamp::parse(value).map_err(|_| AssemblyError::InvalidEvidenceRelationship)
}

fn surface_text(value: capture::ClientSurface) -> &'static str {
    match value {
        capture::ClientSurface::Cli => "cli",
        capture::ClientSurface::Desktop => "desktop",
        capture::ClientSurface::Unknown => "unknown",
    }
}

fn file_change_text(value: domain::FileChangeKind) -> &'static str {
    match value {
        domain::FileChangeKind::Added => "added",
        domain::FileChangeKind::Modified => "modified",
        domain::FileChangeKind::Deleted => "deleted",
        domain::FileChangeKind::Renamed => "renamed",
        domain::FileChangeKind::Unknown => "unknown",
    }
}

fn deterministic_uuid(scope: &str, parts: &[&str]) -> Uuid {
    let mut digest = Sha256::new();
    digest.update(b"mochi-deterministic-uuid-v1");
    update_part(&mut digest, scope.as_bytes());
    for part in parts {
        update_part(&mut digest, part.as_bytes());
    }
    let bytes = digest.finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&bytes[..16]);
    id[6] = (id[6] & 0x0f) | 0x80;
    id[8] = (id[8] & 0x3f) | 0x80;
    Uuid::from_bytes(id)
}

fn hash_parts(prefix: &[u8], parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(prefix);
    for part in parts {
        update_part(&mut digest, part.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

fn update_part(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}
