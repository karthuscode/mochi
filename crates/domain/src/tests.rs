use super::*;

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("synthetic timestamp is valid")
}

fn project_id() -> ProjectId {
    ProjectId::parse("10000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn session_id() -> SessionId {
    SessionId::parse("20000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn turn_id() -> TurnId {
    TurnId::parse("30000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn event_id() -> EventId {
    EventId::parse("40000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn tool_id() -> ToolExecutionId {
    ToolExecutionId::parse("50000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn command_id() -> CommandExecutionId {
    CommandExecutionId::parse("60000000-0000-4000-8000-000000000001").expect("fixture UUID")
}

fn capabilities(surface: ClientSurface) -> CaptureCapabilities {
    CaptureCapabilities {
        session_lifecycle: CapabilitySupport::Supported,
        user_prompt: if surface == ClientSurface::Desktop {
            CapabilitySupport::Unknown
        } else {
            CapabilitySupport::Supported
        },
        agent_response: CapabilitySupport::Supported,
        tool_activity: CapabilitySupport::Supported,
        commands: CapabilitySupport::Supported,
        file_activity: CapabilitySupport::Partial,
        permissions: CapabilitySupport::Supported,
        interrupts: if surface == ClientSurface::Desktop {
            CapabilitySupport::Unknown
        } else {
            CapabilitySupport::Supported
        },
        git_context: CapabilitySupport::Supported,
    }
}

fn complete_capture() -> CaptureCompleteness {
    CaptureCompleteness {
        session_lifecycle: EvidenceCompleteness::Complete,
        prompt: EvidenceCompleteness::Complete,
        agent_response: EvidenceCompleteness::Complete,
        tools: EvidenceCompleteness::Complete,
        commands: EvidenceCompleteness::Complete,
        files: EvidenceCompleteness::Complete,
        git: EvidenceCompleteness::Complete,
        interrupts: EvidenceCompleteness::NotApplicable,
    }
}

fn source(surface: ClientSurface) -> SourceDescriptor {
    SourceDescriptor {
        provider: "codex".to_owned(),
        client_surface: surface,
        client_version: Some("0.151.0".to_owned()),
        adapter_version: "prototype-1".to_owned(),
        transport: "hooks".to_owned(),
    }
}

fn event() -> SessionEvent {
    SessionEvent {
        schema_version: SESSION_EVENT_SCHEMA_VERSION,
        id: event_id(),
        session_id: session_id(),
        project_id: project_id(),
        turn_id: Some(turn_id()),
        source: source(ClientSurface::Cli),
        provenance: EventProvenance {
            source_event_id: Some("synthetic-prompt-1".to_owned()),
            source_sequence: None,
            source_event_type: Some("UserPromptSubmit".to_owned()),
            occurred_at: Some(timestamp("2026-09-17T10:00:01Z")),
        },
        received_at: timestamp("2026-09-17T10:00:02Z"),
        sequence: 1,
        origin: EventOrigin::Provider,
        event: SessionEventData::UserPrompt(UserPromptPayload {
            text: "Add a bounded parser.".to_owned(),
        }),
        sensitivity: Sensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: 0,
            rules_version: "prototype-1".to_owned(),
            policy_revision: 1,
            truncated: false,
        },
    }
}

fn turn(prompt: Option<&str>, source_turn_id: Option<&str>) -> SessionTurn {
    SessionTurn {
        id: turn_id(),
        session_id: session_id(),
        source_turn_id: source_turn_id.map(str::to_owned),
        started_at: Some(timestamp("2026-09-17T10:00:00Z")),
        ended_at: Some(timestamp("2026-09-17T10:01:00Z")),
        user_prompt: prompt.map(str::to_owned),
        agent_final_response: Some("Implemented the parser.".to_owned()),
        event_ids: vec![event_id()],
        status: TurnStatus::Completed,
    }
}

fn unavailable_git() -> GitContext {
    GitContext::Unavailable {
        reason: GitUnavailableReason::NotRepository,
    }
}

fn git_snapshot(state: WorkingTreeState, truncated: bool) -> GitSnapshot {
    GitSnapshot {
        repository_root: "/synthetic/mochi-fixture".to_owned(),
        branch: Some("fixture".to_owned()),
        detached: Some(false),
        head_commit: Some("0123456789abcdef".to_owned()),
        captured_at: timestamp("2026-09-17T10:00:00Z"),
        working_tree_state: state,
        files: if state == WorkingTreeState::Dirty {
            vec![GitFileState {
                path: "src/preexisting.ts".to_owned(),
                previous_path: None,
                change_type: FileChangeKind::Modified,
                staged: Some(false),
                unstaged: Some(true),
                content_hash: Some("synthetic-hash".to_owned()),
                content: Some("export const synthetic = true;".to_owned()),
                byte_count: Some(42),
                omission: None,
                truncated: false,
            }]
        } else {
            Vec::new()
        },
        diff_stats: Some(GitDiffStats {
            files_changed: if state == WorkingTreeState::Dirty {
                1
            } else {
                0
            },
            insertions: None,
            deletions: None,
        }),
        excluded_count: 0,
        omitted_file_count: 0,
        truncated,
        warnings: if truncated {
            vec!["file limit reached".to_owned()]
        } else {
            Vec::new()
        },
    }
}

fn session_data(status: SessionStatus) -> CodingSessionData {
    let ended_at = if status == SessionStatus::Active || status == SessionStatus::Incomplete {
        None
    } else {
        Some(timestamp("2026-09-17T10:02:00Z"))
    };
    CodingSessionData {
        schema_version: CODING_SESSION_SCHEMA_VERSION,
        id: session_id(),
        project_id: project_id(),
        source: source(ClientSurface::Cli),
        started_at: timestamp("2026-09-17T10:00:00Z"),
        ended_at,
        status,
        turns: vec![turn(Some("Add a bounded parser."), Some("source-turn-1"))],
        events: vec![event()],
        tool_executions: Vec::new(),
        command_executions: Vec::new(),
        file_changes: Vec::new(),
        git_context: unavailable_git(),
        capture_capabilities: capabilities(ClientSurface::Cli),
        capture_completeness: complete_capture(),
    }
}

#[test]
fn project_identity_is_distinct_from_mutable_display_name() {
    let project = Project {
        id: project_id(),
        display_name: "Synthetic Workspace".to_owned(),
        root_path: "/synthetic/workspace".to_owned(),
        repository_identity: Some(RepositoryIdentity {
            kind: RepositoryKind::Git,
            opaque_id: "fixture-repository-id".to_owned(),
        }),
        created_at: timestamp("2026-09-17T09:00:00Z"),
        last_seen_at: timestamp("2026-09-17T10:00:00Z"),
    };
    assert_eq!(project.validate(), Ok(()));
}

#[test]
fn coding_session_accepts_active_completed_and_optional_git() {
    let active = CodingSession::new(session_data(SessionStatus::Active)).expect("active session");
    let completed =
        CodingSession::new(session_data(SessionStatus::Completed)).expect("completed session");
    assert_eq!(active.data().ended_at, None);
    assert!(matches!(
        completed.data().git_context,
        GitContext::Unavailable { .. }
    ));
}

#[test]
fn coding_session_rejects_invalid_time_and_missing_completed_end() {
    let mut reversed = session_data(SessionStatus::Completed);
    reversed.ended_at = Some(timestamp("2026-09-17T09:59:59Z"));
    assert!(matches!(
        CodingSession::new(reversed),
        Err(DomainError::InvalidTimeRange { .. })
    ));

    let mut missing_end = session_data(SessionStatus::Completed);
    missing_end.ended_at = None;
    assert!(matches!(
        CodingSession::new(missing_end),
        Err(DomainError::InvalidStatus { .. })
    ));
}

#[test]
fn missing_prompt_and_source_turn_identity_are_valid() {
    let mut no_prompt = session_data(SessionStatus::Completed);
    no_prompt.turns = vec![turn(None, None)];
    assert!(CodingSession::new(no_prompt).is_ok());
}

#[test]
fn desktop_capabilities_and_partial_capture_preserve_uncertainty() {
    let mut desktop = session_data(SessionStatus::Incomplete);
    desktop.source = source(ClientSurface::Desktop);
    desktop.capture_capabilities = capabilities(ClientSurface::Desktop);
    desktop.capture_completeness.prompt = EvidenceCompleteness::Unknown;
    desktop.capture_completeness.interrupts = EvidenceCompleteness::Unknown;
    desktop.capture_completeness.session_lifecycle = EvidenceCompleteness::Partial;
    desktop.turns = vec![turn(None, None)];
    let desktop = CodingSession::new(desktop).expect("partial Desktop fixture");
    assert_eq!(
        desktop.data().capture_capabilities.user_prompt,
        CapabilitySupport::Unknown
    );
    assert_eq!(desktop.overall_completeness(), OverallCompleteness::Partial);
}

#[test]
fn completeness_is_derived_deterministically() {
    let complete = complete_capture();
    assert_eq!(complete.overall(), OverallCompleteness::Complete);
    let mut partial = complete.clone();
    partial.prompt = EvidenceCompleteness::Unknown;
    assert_eq!(partial.overall(), OverallCompleteness::Partial);
    partial.session_lifecycle = EvidenceCompleteness::Missing;
    assert_eq!(partial.overall(), OverallCompleteness::Degraded);
}

#[test]
fn tool_lifecycle_supports_completed_failed_interrupted_and_incomplete() {
    for status in [
        ToolExecutionStatus::Completed,
        ToolExecutionStatus::Failed,
        ToolExecutionStatus::Interrupted,
        ToolExecutionStatus::Incomplete,
    ] {
        let terminal = matches!(
            status,
            ToolExecutionStatus::Completed | ToolExecutionStatus::Failed
        );
        let tool = ToolExecution {
            id: tool_id(),
            session_id: session_id(),
            turn_id: Some(turn_id()),
            source_tool_use_id: Some("synthetic-tool-1".to_owned()),
            tool_name: "Bash".to_owned(),
            started_at: Some(timestamp("2026-09-17T10:00:10Z")),
            completed_at: terminal.then(|| timestamp("2026-09-17T10:00:20Z")),
            status,
            input_summary: Some("Run synthetic checks".to_owned()),
            output_summary: None,
            related_file_paths: vec!["src/parser.ts".to_owned()],
        };
        assert_eq!(tool.validate(), Ok(()));
    }
}

#[test]
fn command_exit_code_is_optional_and_interruption_is_not_success() {
    for (status, exit_code, completed_at) in [
        (
            CommandExecutionStatus::Completed,
            Some(0),
            Some(timestamp("2026-09-17T10:00:20Z")),
        ),
        (CommandExecutionStatus::Unknown, None, None),
        (CommandExecutionStatus::Interrupted, None, None),
    ] {
        let command = CommandExecution {
            id: command_id(),
            session_id: session_id(),
            turn_id: Some(turn_id()),
            tool_execution_id: None,
            command: "pnpm test".to_owned(),
            working_directory: Some(".".to_owned()),
            started_at: Some(timestamp("2026-09-17T10:00:10Z")),
            completed_at,
            status,
            exit_code,
            output_summary: None,
        };
        assert_eq!(command.validate(), Ok(()));
    }
}

#[test]
fn git_context_supports_clean_dirty_unavailable_and_truncated_evidence() {
    let clean = git_snapshot(WorkingTreeState::Clean, false);
    clean.validate().expect("clean baseline");
    let dirty = git_snapshot(WorkingTreeState::Dirty, false);
    assert_eq!(dirty.files[0].path, "src/preexisting.ts");
    let truncated = git_snapshot(WorkingTreeState::Dirty, true);
    assert!(truncated.truncated);
    assert_eq!(unavailable_git().validate(), Ok(()));
}

#[test]
fn before_and_after_git_snapshots_preserve_preexisting_dirty_state() {
    let before = git_snapshot(WorkingTreeState::Dirty, false);
    let mut after = before.clone();
    after.captured_at = timestamp("2026-09-17T10:02:00Z");
    after.files.push(GitFileState {
        path: "src/new.ts".to_owned(),
        previous_path: None,
        change_type: FileChangeKind::Added,
        staged: Some(false),
        unstaged: Some(true),
        content_hash: Some("synthetic-new-hash".to_owned()),
        content: Some("export const added = true;".to_owned()),
        byte_count: Some(18),
        omission: None,
        truncated: false,
    });
    let context = GitContext::Available {
        before: Box::new(before),
        after: Some(Box::new(after)),
    };
    assert_eq!(context.validate(), Ok(()));
}

#[test]
fn file_change_records_observation_separately_from_attribution() {
    let change = FileChange {
        id: FileChangeId::parse("70000000-0000-4000-8000-000000000001").expect("fixture UUID"),
        path: "src/parser.ts".to_owned(),
        previous_path: None,
        change_type: FileChangeKind::Modified,
        source: FileChangeSource::GitComparison,
        observed_during_session: true,
        attribution: FileAttribution::Ambiguous,
    };
    assert_eq!(change.validate(), Ok(()));
}

#[test]
fn coding_session_round_trips_through_validated_json() {
    let session =
        CodingSession::new(session_data(SessionStatus::Completed)).expect("fixture session");
    let json = session.to_json().expect("serialize");
    let parsed = CodingSession::from_json(&json).expect("deserialize and validate");
    assert_eq!(parsed, session);
}

#[test]
fn unsupported_schema_and_impossible_transition_are_rejected() {
    let mut data = session_data(SessionStatus::Completed);
    data.schema_version = 2;
    assert!(matches!(
        CodingSession::new(data),
        Err(DomainError::UnsupportedSchemaVersion { found: 2 })
    ));

    let completed = CodingSession::new(session_data(SessionStatus::Completed)).expect("completed");
    assert_eq!(
        completed.transition_status(SessionStatus::Active),
        Err(DomainError::InvalidStatusTransition)
    );
}

#[test]
fn utc_timestamp_rejects_local_offsets() {
    assert!(UtcTimestamp::parse("2026-09-17T12:00:00+02:00").is_err());
}

#[test]
fn shared_typescript_fixture_matches_authoritative_rust_contract() {
    let fixture =
        include_str!("../../../packages/domain/fixtures/coding-session-cli-complete.json");
    let session = CodingSession::from_json(fixture).expect("shared fixture validates in Rust");
    assert_eq!(session.data().status, SessionStatus::Completed);
    assert_eq!(session.data().tool_executions.len(), 1);
    assert_eq!(session.data().command_executions[0].exit_code, Some(0));
}
