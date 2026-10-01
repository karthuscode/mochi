use super::*;
use mochi_capture::{
    Spool,
    model::{
        AgentMessagePayload, CapabilitySupport as SourceSupport, CaptureGapPayload,
        CaptureGapReason, ClientSurface as CaptureSurface, CommandExecutedPayload,
        CommandResultPayload, CompletionStatus, EventOrigin as CaptureOrigin, MessageKind,
        NormalizedEvent, PendingEvent, Sensitivity as CaptureSensitivity,
        SensitivityClassification, SessionSourceCapabilities, SessionStartReason,
        SessionStartedPayload, SessionStopReason, SessionStoppedPayload,
        SourceDescriptor as CaptureSource, ToolCompletedPayload, ToolStartedPayload,
        TurnCompletedPayload, UserPromptPayload,
    },
    spool::SpoolLimits,
};
use mochi_domain::{
    CapabilitySupport, EvidenceCompleteness, FileAttribution, FileChangeKind, GitContext,
    GitFileState, GitSnapshot, GitUnavailableReason, Project, ProjectId, SessionStatus,
    ToolExecutionStatus, TurnStatus, UtcTimestamp, WorkingTreeState,
};
use mochi_persistence::{
    AssemblyEvidenceLoad, AssemblyRepository, AssemblySourceKey, AssemblyWrite, CapturePolicy,
    Clock, CodingSessionRepository, IngressAssemblyState, IngressRepository, ProjectRepository,
    SqliteStore, StorageError, StorageResult,
};
use std::{sync::Arc, time::Instant};
use tempfile::TempDir;
use uuid::Uuid;

const NOW: &str = "2026-09-18T10:00:00Z";
const PROJECT_ID: &str = "10000000-0000-4000-8000-000000000001";

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now_rfc3339(&self) -> StorageResult<String> {
        Ok(NOW.to_owned())
    }
}

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("fixture timestamp")
}

fn project_id() -> ProjectId {
    ProjectId::parse(PROJECT_ID).expect("fixture project")
}

fn open_store(root: &TempDir) -> SqliteStore {
    let store = SqliteStore::open(root.path().join("app/mochi.sqlite3"), Arc::new(FixedClock))
        .expect("open store");
    store
        .create_project(
            &Project {
                id: project_id(),
                display_name: "Synthetic project".to_owned(),
                root_path: root.path().join("repo").display().to_string(),
                repository_identity: None,
                created_at: timestamp(NOW),
                last_seen_at: timestamp(NOW),
            },
            CapturePolicy {
                tracking_enabled: true,
                revision: 1,
            },
        )
        .expect("create project");
    store
}

fn source(surface: CaptureSurface) -> CaptureSource {
    CaptureSource {
        provider: "codex".to_owned(),
        adapter_version: "0.1.0".to_owned(),
        transport: "hooks".to_owned(),
        client_surface: surface,
    }
}

fn capabilities(surface: CaptureSurface) -> SessionSourceCapabilities {
    SessionSourceCapabilities {
        identity: SourceSupport::Supported,
        project_association: SourceSupport::Supported,
        activity: SourceSupport::Supported,
        prompts: if surface == CaptureSurface::Cli {
            SourceSupport::Supported
        } else {
            SourceSupport::Unknown
        },
        agent_messages: SourceSupport::Supported,
        tool_lifecycle: SourceSupport::Supported,
        permissions: SourceSupport::Supported,
        interrupts: if surface == CaptureSurface::Cli {
            SourceSupport::Supported
        } else {
            SourceSupport::Unknown
        },
        commands: SourceSupport::Supported,
        file_events: SourceSupport::Unknown,
        session_end: SourceSupport::Supported,
    }
}

fn pending(
    id: &str,
    session: Option<&str>,
    turn: Option<&str>,
    tool: Option<&str>,
    event: NormalizedEvent,
) -> PendingEvent {
    PendingEvent {
        source_event_id: Some(id.to_owned()),
        source_event_type: id.to_owned(),
        external_session_id: session.map(str::to_owned),
        external_turn_id: turn.map(str::to_owned),
        external_tool_use_id: tool.map(str::to_owned),
        event,
        sensitivity: CaptureSensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: 0,
            rules_version: "prototype-1".to_owned(),
            policy_revision: 1,
            truncated: false,
        },
    }
}

fn started(id: &str, session: &str, surface: CaptureSurface) -> PendingEvent {
    pending(
        id,
        Some(session),
        None,
        None,
        NormalizedEvent::SessionStarted(SessionStartedPayload {
            reason: SessionStartReason::Startup,
            capabilities: capabilities(surface),
            start_boundary_known: true,
        }),
    )
}

fn stopped(id: &str, session: &str, reason: SessionStopReason) -> PendingEvent {
    pending(
        id,
        Some(session),
        None,
        None,
        NormalizedEvent::SessionStopped(SessionStoppedPayload {
            reason,
            end_boundary_known: true,
        }),
    )
}

fn complete_cli(session: &str) -> Vec<PendingEvent> {
    vec![
        started("start", session, CaptureSurface::Cli),
        pending(
            "prompt",
            Some(session),
            Some("turn-1"),
            None,
            NormalizedEvent::UserPrompt(UserPromptPayload {
                text: "Run the synthetic checks.".to_owned(),
                turn_ref: Some("turn-1".to_owned()),
            }),
        ),
        pending(
            "tool-start",
            Some(session),
            Some("turn-1"),
            Some("tool-1"),
            NormalizedEvent::ToolStarted(ToolStartedPayload {
                tool_call_ref: "tool-1".to_owned(),
                tool_kind: "Bash".to_owned(),
                summary: None,
                turn_ref: Some("turn-1".to_owned()),
            }),
        ),
        pending(
            "command-start",
            Some(session),
            Some("turn-1"),
            Some("tool-1"),
            NormalizedEvent::CommandExecuted(CommandExecutedPayload {
                command_ref: "tool-1".to_owned(),
                display: "cargo test".to_owned(),
                working_directory: None,
            }),
        ),
        pending(
            "tool-end",
            Some(session),
            Some("turn-1"),
            Some("tool-1"),
            NormalizedEvent::ToolCompleted(ToolCompletedPayload {
                tool_call_ref: "tool-1".to_owned(),
                status: CompletionStatus::Succeeded,
                summary: Some("completed".to_owned()),
                duration_ms: None,
            }),
        ),
        pending(
            "command-end",
            Some(session),
            Some("turn-1"),
            Some("tool-1"),
            NormalizedEvent::CommandResult(CommandResultPayload {
                command_ref: "tool-1".to_owned(),
                exit_code: Some(0),
                output: Some("ok".to_owned()),
                duration_ms: None,
            }),
        ),
        pending(
            "response",
            Some(session),
            Some("turn-1"),
            None,
            NormalizedEvent::AgentMessage(AgentMessagePayload {
                text: "The synthetic checks pass.".to_owned(),
                turn_ref: Some("turn-1".to_owned()),
                message_kind: MessageKind::Final,
            }),
        ),
        pending(
            "turn-end",
            Some(session),
            Some("turn-1"),
            None,
            NormalizedEvent::TurnCompleted(TurnCompletedPayload {
                turn_ref: Some("turn-1".to_owned()),
                status: CompletionStatus::Succeeded,
            }),
        ),
        stopped("stop", session, SessionStopReason::Unknown),
    ]
}

fn import(store: &SqliteStore, root: &TempDir, surface: CaptureSurface, events: Vec<PendingEvent>) {
    let spool = Spool::new(root.path().join("spool"), SpoolLimits::default());
    spool
        .append(project_id().as_uuid(), source(surface), NOW, events)
        .expect("append evidence");
    let outcome = store
        .import_spool_batch(&spool, 100)
        .expect("import evidence");
    assert!(outcome.inserted > 0);
}

#[test]
fn cli_complete_session_is_assembled_and_associated_atomically() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    import(
        &store,
        &spool_root,
        CaptureSurface::Cli,
        complete_cli("cli-complete"),
    );
    let outcome = SessionAssemblyEngine::new(&store)
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap();
    assert_eq!(outcome.assembled_session_ids.len(), 1);
    let session = store
        .get_session(outcome.assembled_session_ids[0])
        .unwrap()
        .unwrap();
    let data = session.data();
    assert_eq!(data.status, SessionStatus::Completed);
    assert_eq!(data.turns.len(), 1);
    assert_eq!(
        data.turns[0].user_prompt.as_deref(),
        Some("Run the synthetic checks.")
    );
    assert_eq!(data.turns[0].status, TurnStatus::Completed);
    assert_eq!(data.tool_executions.len(), 1);
    assert_eq!(
        data.tool_executions[0].status,
        ToolExecutionStatus::Completed
    );
    assert_eq!(data.command_executions[0].exit_code, Some(0));
    assert_eq!(
        data.capture_completeness.session_lifecycle,
        EvidenceCompleteness::Complete
    );
    assert_eq!(
        data.capture_completeness.prompt,
        EvidenceCompleteness::Complete
    );
    let ingress = store.list_session_ingress(data.id, 100).unwrap();
    assert_eq!(ingress.len(), complete_cli("unused").len());
    assert!(ingress.iter().all(|id| matches!(
        store.ingress_assembly_state(*id).unwrap(),
        IngressAssemblyState::Assigned { session_id } if session_id == data.id
    )));
}

#[test]
fn failed_then_recovered_commands_are_both_retained() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    let mut events = complete_cli("recovery");
    events.retain(|event| {
        !matches!(
            event.event,
            NormalizedEvent::ToolStarted(_)
                | NormalizedEvent::ToolCompleted(_)
                | NormalizedEvent::CommandExecuted(_)
                | NormalizedEvent::CommandResult(_)
        )
    });
    for (suffix, exit_code, status) in [
        ("failed", 1, CompletionStatus::Failed),
        ("passed", 0, CompletionStatus::Succeeded),
    ] {
        let reference = format!("tool-{suffix}");
        events.insert(
            events.len() - 3,
            pending(
                &format!("tool-start-{suffix}"),
                Some("recovery"),
                Some("turn-1"),
                Some(&reference),
                NormalizedEvent::ToolStarted(ToolStartedPayload {
                    tool_call_ref: reference.clone(),
                    tool_kind: "Bash".to_owned(),
                    summary: None,
                    turn_ref: Some("turn-1".to_owned()),
                }),
            ),
        );
        events.insert(
            events.len() - 3,
            pending(
                &format!("command-start-{suffix}"),
                Some("recovery"),
                Some("turn-1"),
                Some(&reference),
                NormalizedEvent::CommandExecuted(CommandExecutedPayload {
                    command_ref: reference.clone(),
                    display: "cargo test".to_owned(),
                    working_directory: None,
                }),
            ),
        );
        events.insert(
            events.len() - 3,
            pending(
                &format!("tool-end-{suffix}"),
                Some("recovery"),
                Some("turn-1"),
                Some(&reference),
                NormalizedEvent::ToolCompleted(ToolCompletedPayload {
                    tool_call_ref: reference.clone(),
                    status,
                    summary: None,
                    duration_ms: None,
                }),
            ),
        );
        events.insert(
            events.len() - 3,
            pending(
                &format!("command-end-{suffix}"),
                Some("recovery"),
                Some("turn-1"),
                Some(&reference),
                NormalizedEvent::CommandResult(CommandResultPayload {
                    command_ref: reference.clone(),
                    exit_code: Some(exit_code),
                    output: None,
                    duration_ms: None,
                }),
            ),
        );
    }
    import(&store, &spool_root, CaptureSurface::Cli, events);
    let id = SessionAssemblyEngine::new(&store)
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap()
        .assembled_session_ids[0];
    let session = store.get_session(id).unwrap().unwrap();
    assert_eq!(session.data().command_executions.len(), 2);
    assert_eq!(session.data().command_executions[0].exit_code, Some(1));
    assert_eq!(session.data().command_executions[1].exit_code, Some(0));
}

#[test]
fn interrupted_and_missing_end_sessions_preserve_uncertainty() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    let interrupted = vec![
        started("i-start", "interrupted", CaptureSurface::Cli),
        pending(
            "i-tool",
            Some("interrupted"),
            Some("turn-i"),
            Some("tool-i"),
            NormalizedEvent::ToolStarted(ToolStartedPayload {
                tool_call_ref: "tool-i".to_owned(),
                tool_kind: "Bash".to_owned(),
                summary: None,
                turn_ref: Some("turn-i".to_owned()),
            }),
        ),
        pending(
            "i-command",
            Some("interrupted"),
            Some("turn-i"),
            Some("tool-i"),
            NormalizedEvent::CommandExecuted(CommandExecutedPayload {
                command_ref: "tool-i".to_owned(),
                display: "sleep 30".to_owned(),
                working_directory: None,
            }),
        ),
        pending(
            "i-turn",
            Some("interrupted"),
            Some("turn-i"),
            None,
            NormalizedEvent::TurnCompleted(TurnCompletedPayload {
                turn_ref: Some("turn-i".to_owned()),
                status: CompletionStatus::Cancelled,
            }),
        ),
        stopped("i-stop", "interrupted", SessionStopReason::Interrupted),
        started("m-start", "missing-end", CaptureSurface::Cli),
        pending(
            "m-prompt",
            Some("missing-end"),
            Some("turn-m"),
            None,
            NormalizedEvent::UserPrompt(UserPromptPayload {
                text: "Keep this incomplete.".to_owned(),
                turn_ref: Some("turn-m".to_owned()),
            }),
        ),
    ];
    import(&store, &spool_root, CaptureSurface::Cli, interrupted);
    let ids = SessionAssemblyEngine::new(&store)
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap()
        .assembled_session_ids;
    assert_eq!(ids.len(), 2);
    let sessions = ids
        .into_iter()
        .map(|id| store.get_session(id).unwrap().unwrap())
        .collect::<Vec<_>>();
    let interrupted = sessions
        .iter()
        .find(|session| session.data().status == SessionStatus::Interrupted)
        .unwrap();
    assert_eq!(
        interrupted.data().tool_executions[0].status,
        ToolExecutionStatus::Interrupted
    );
    assert_eq!(interrupted.data().command_executions[0].exit_code, None);
    let incomplete = sessions
        .iter()
        .find(|session| session.data().status == SessionStatus::Incomplete)
        .unwrap();
    assert!(incomplete.data().ended_at.is_none());
    assert_ne!(
        incomplete.data().capture_completeness.session_lifecycle,
        EvidenceCompleteness::Complete
    );
}

#[test]
fn desktop_unknowns_concurrent_sessions_and_explicit_gaps_remain_separate() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    let events = vec![
        started("a-start", "session-a", CaptureSurface::Desktop),
        started("b-start", "session-b", CaptureSurface::Desktop),
        pending(
            "a-response",
            Some("session-a"),
            Some("turn-a"),
            None,
            NormalizedEvent::AgentMessage(AgentMessagePayload {
                text: "A response".to_owned(),
                turn_ref: Some("turn-a".to_owned()),
                message_kind: MessageKind::Final,
            }),
        ),
        pending(
            "b-gap",
            Some("session-b"),
            Some("turn-b"),
            None,
            NormalizedEvent::CaptureGap(CaptureGapPayload {
                reason: CaptureGapReason::Overflow,
                from: None,
                to: None,
                dropped_count: Some(1),
            }),
        ),
        stopped("a-stop", "session-a", SessionStopReason::Unknown),
        stopped("b-stop", "session-b", SessionStopReason::Unknown),
    ];
    import(&store, &spool_root, CaptureSurface::Desktop, events);
    let ids = SessionAssemblyEngine::new(&store)
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap()
        .assembled_session_ids;
    assert_eq!(ids.len(), 2);
    let sessions = ids
        .into_iter()
        .map(|id| store.get_session(id).unwrap().unwrap())
        .collect::<Vec<_>>();
    for session in &sessions {
        assert_eq!(
            session.data().capture_capabilities.user_prompt,
            CapabilitySupport::Unknown
        );
        assert_eq!(
            session.data().capture_capabilities.interrupts,
            CapabilitySupport::Unknown
        );
        assert_eq!(
            session.data().capture_completeness.prompt,
            EvidenceCompleteness::Unknown
        );
    }
    let a = sessions
        .iter()
        .find(|session| {
            session
                .data()
                .turns
                .iter()
                .any(|turn| turn.agent_final_response.as_deref() == Some("A response"))
        })
        .unwrap();
    assert!(
        a.data()
            .events
            .iter()
            .all(|event| !matches!(event.event, mochi_domain::SessionEventData::CaptureGap(_)))
    );
    let b = sessions
        .iter()
        .find(|session| session.data().id != a.data().id)
        .unwrap();
    assert!(
        b.data()
            .events
            .iter()
            .any(|event| matches!(event.event, mochi_domain::SessionEventData::CaptureGap(_)))
    );
    assert_eq!(
        b.data().capture_completeness.session_lifecycle,
        EvidenceCompleteness::Partial
    );
}

#[test]
fn explicit_git_context_preserves_dirty_baseline_and_no_git_is_valid() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    import(
        &store,
        &spool_root,
        CaptureSurface::Cli,
        complete_cli("git-session"),
    );
    let candidate = store.list_assembly_candidates(Some(1)).unwrap().remove(0);
    let before = git_snapshot("2026-09-18T09:59:00Z", "a-old", "b-old");
    let after = git_snapshot("2026-09-18T10:01:00Z", "a-old", "b-new");
    let context = GitContext::Available {
        before: Box::new(before),
        after: Some(Box::new(after)),
    };
    let id = SessionAssemblyEngine::new(&store)
        .assemble_pending(
            AssemblyLimits::default(),
            &[ExplicitGitContext {
                source: candidate.source,
                context,
            }],
        )
        .unwrap()
        .assembled_session_ids[0];
    let session = store.get_session(id).unwrap().unwrap();
    assert_eq!(session.data().file_changes.len(), 1);
    assert_eq!(session.data().file_changes[0].path, "src/b.rs");
    assert_eq!(
        session.data().file_changes[0].attribution,
        FileAttribution::Ambiguous
    );

    let other_root = TempDir::new().unwrap();
    let other_spool = TempDir::new().unwrap();
    let other = open_store(&other_root);
    import(
        &other,
        &other_spool,
        CaptureSurface::Cli,
        complete_cli("no-git"),
    );
    let candidate = other.list_assembly_candidates(Some(1)).unwrap().remove(0);
    let id = SessionAssemblyEngine::new(&other)
        .assemble_pending(
            AssemblyLimits::default(),
            &[ExplicitGitContext {
                source: candidate.source,
                context: GitContext::Unavailable {
                    reason: GitUnavailableReason::NotRepository,
                },
            }],
        )
        .unwrap()
        .assembled_session_ids[0];
    let session = other.get_session(id).unwrap().unwrap();
    assert_eq!(
        session.data().capture_completeness.git,
        EvidenceCompleteness::NotApplicable
    );
}

#[test]
fn duplicate_runs_and_deleted_sessions_do_not_reassemble() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    import(
        &store,
        &spool_root,
        CaptureSurface::Cli,
        complete_cli("once"),
    );
    let engine = SessionAssemblyEngine::new(&store);
    let first = engine
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap();
    let id = first.assembled_session_ids[0];
    let second = engine
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap();
    assert!(second.assembled_session_ids.is_empty());
    assert!(store.delete_session(id).unwrap());
    let third = engine
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap();
    assert!(third.assembled_session_ids.is_empty());
    assert!(store.get_session(id).unwrap().is_none());
}

#[test]
fn association_constraint_failure_rolls_back_the_session_insert() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    let mut events = complete_cli("atomic-a");
    events.extend(complete_cli("atomic-b"));
    import(&store, &spool_root, CaptureSurface::Cli, events);
    let candidates = store.list_assembly_candidates(Some(2)).unwrap();
    assert_eq!(candidates.len(), 2);

    let load = |source: &AssemblySourceKey| match store
        .load_assembly_evidence(source, 100, 1024 * 1024)
        .unwrap()
    {
        AssemblyEvidenceLoad::Ready(value) => value,
        AssemblyEvidenceLoad::BoundsExceeded { .. } => panic!("fixture must fit"),
    };
    let first_evidence = load(&candidates[0].source);
    let first_session = assemble_session(&candidates[0].source, &first_evidence, None).unwrap();
    let collision_key = assembly_group_key(&candidates[0].source);
    store
        .persist_assembled_session(AssemblyWrite {
            group_key: &collision_key,
            source: &candidates[0].source,
            session: &first_session,
            ingress_ids: &first_evidence
                .iter()
                .map(|value| value.record.id)
                .collect::<Vec<_>>(),
            assembly_version: ASSEMBLY_VERSION,
            evidence_fingerprint: &evidence_fingerprint(&first_evidence),
        })
        .unwrap();

    let second_evidence = load(&candidates[1].source);
    let second_session = assemble_session(&candidates[1].source, &second_evidence, None).unwrap();
    let result = store.persist_assembled_session(AssemblyWrite {
        group_key: &collision_key,
        source: &candidates[1].source,
        session: &second_session,
        ingress_ids: &second_evidence
            .iter()
            .map(|value| value.record.id)
            .collect::<Vec<_>>(),
        assembly_version: ASSEMBLY_VERSION,
        evidence_fingerprint: &evidence_fingerprint(&second_evidence),
    });
    assert_eq!(result, Err(StorageError::Constraint));
    assert!(
        store
            .get_session(second_session.data().id)
            .unwrap()
            .is_none()
    );
    assert!(second_evidence.iter().all(|item| {
        store.ingress_assembly_state(item.record.id).unwrap() == IngressAssemblyState::Unassigned
    }));
}

#[test]
fn unidentified_ambiguous_and_oversized_evidence_fail_safely() {
    let root = TempDir::new().unwrap();
    let spool_root = TempDir::new().unwrap();
    let store = open_store(&root);
    import(
        &store,
        &spool_root,
        CaptureSurface::Cli,
        vec![pending(
            "unknown-session",
            None,
            None,
            None,
            NormalizedEvent::CaptureGap(CaptureGapPayload {
                reason: CaptureGapReason::InvalidInput,
                from: None,
                to: None,
                dropped_count: None,
            }),
        )],
    );
    let outcome = SessionAssemblyEngine::new(&store)
        .assemble_pending(AssemblyLimits::default(), &[])
        .unwrap();
    assert_eq!(outcome.ignored_ingress, 1);

    let ambiguous_root = TempDir::new().unwrap();
    let ambiguous_spool = TempDir::new().unwrap();
    let ambiguous = open_store(&ambiguous_root);
    import(
        &ambiguous,
        &ambiguous_spool,
        CaptureSurface::Cli,
        vec![
            started("start", "ambiguous", CaptureSurface::Cli),
            stopped("stop-1", "ambiguous", SessionStopReason::Unknown),
            stopped("stop-2", "ambiguous", SessionStopReason::Unknown),
        ],
    );
    assert_eq!(
        SessionAssemblyEngine::new(&ambiguous).assemble_pending(AssemblyLimits::default(), &[]),
        Err(AssemblyError::AmbiguousAssociation)
    );
    assert!(
        ambiguous
            .list_session_ingress(
                session_id_for_source(
                    &ambiguous.list_assembly_candidates(Some(1)).unwrap()[0].source
                ),
                100
            )
            .unwrap()
            .is_empty()
    );

    let bounded_root = TempDir::new().unwrap();
    let bounded_spool = TempDir::new().unwrap();
    let bounded = open_store(&bounded_root);
    import(
        &bounded,
        &bounded_spool,
        CaptureSurface::Cli,
        complete_cli("bounded"),
    );
    let outcome = SessionAssemblyEngine::new(&bounded)
        .assemble_pending(
            AssemblyLimits {
                ingress_records_per_session: 2,
                ..AssemblyLimits::default()
            },
            &[],
        )
        .unwrap();
    assert_eq!(outcome.deferred_groups, 1);
    assert!(outcome.assembled_session_ids.is_empty());
}

#[test]
fn representative_pure_assembly_performance_is_practical() {
    for count in [10usize, 100, 500] {
        let evidence = synthetic_evidence(count);
        let source = AssemblySourceKey {
            project_id: project_id(),
            provider: "codex".to_owned(),
            adapter_version: "0.1.0".to_owned(),
            transport: "hooks".to_owned(),
            client_surface: CaptureSurface::Cli,
            external_session_id: format!("performance-{count}"),
        };
        let started = Instant::now();
        let session = assemble_session(&source, &evidence, None).unwrap();
        let elapsed = started.elapsed();
        eprintln!("assembly fixture {count} events: {elapsed:?}");
        assert_eq!(session.data().events.len(), count);
        assert!(elapsed.as_secs() < 5);
    }
}

fn git_snapshot(at: &str, a_hash: &str, b_hash: &str) -> GitSnapshot {
    GitSnapshot {
        repository_root: "[APPROVED_ROOT]".to_owned(),
        branch: Some("main".to_owned()),
        detached: Some(false),
        head_commit: Some("synthetic-head".to_owned()),
        captured_at: timestamp(at),
        working_tree_state: WorkingTreeState::Dirty,
        files: vec![
            GitFileState {
                path: "src/a.rs".to_owned(),
                previous_path: None,
                change_type: FileChangeKind::Modified,
                staged: Some(false),
                unstaged: Some(true),
                content_hash: Some(a_hash.to_owned()),
                content: Some("synthetic a".to_owned()),
                byte_count: Some(11),
                omission: None,
                truncated: false,
            },
            GitFileState {
                path: "src/b.rs".to_owned(),
                previous_path: None,
                change_type: if b_hash == "b-old" {
                    FileChangeKind::Unknown
                } else {
                    FileChangeKind::Modified
                },
                staged: Some(false),
                unstaged: Some(b_hash != "b-old"),
                content_hash: Some(b_hash.to_owned()),
                content: Some(format!("synthetic {b_hash}")),
                byte_count: Some(15),
                omission: None,
                truncated: false,
            },
        ],
        diff_stats: None,
        excluded_count: 0,
        omitted_file_count: 0,
        truncated: false,
        warnings: Vec::new(),
    }
}

fn synthetic_evidence(count: usize) -> Vec<mochi_persistence::PersistedIngress> {
    (0..count)
        .map(|index| {
            let id = Uuid::from_u128(index as u128 + 1);
            let event = if index == 0 {
                NormalizedEvent::SessionStarted(SessionStartedPayload {
                    reason: SessionStartReason::Startup,
                    capabilities: capabilities(CaptureSurface::Cli),
                    start_boundary_known: true,
                })
            } else if index + 1 == count {
                NormalizedEvent::SessionStopped(SessionStoppedPayload {
                    reason: SessionStopReason::Unknown,
                    end_boundary_known: true,
                })
            } else {
                NormalizedEvent::CaptureGap(CaptureGapPayload {
                    reason: CaptureGapReason::Unsupported,
                    from: None,
                    to: None,
                    dropped_count: None,
                })
            };
            mochi_persistence::PersistedIngress {
                source_identity_key: format!("synthetic-{index}"),
                record: mochi_capture::SpoolIngressRecord {
                    schema_version: mochi_capture::model::INGRESS_SCHEMA_VERSION,
                    id,
                    project_id: project_id().as_uuid(),
                    source: source(CaptureSurface::Cli),
                    source_event_id: Some(format!("event-{index}")),
                    source_sequence: Some(index as u64),
                    source_timestamp: None,
                    received_at: NOW.to_owned(),
                    receive_sequence: index as u64 + 1,
                    source_event_type: "synthetic".to_owned(),
                    external_session_id: Some(format!("performance-{count}")),
                    external_turn_id: None,
                    external_tool_use_id: None,
                    origin: CaptureOrigin::Provider,
                    event,
                    sensitivity: CaptureSensitivity {
                        classification: SensitivityClassification::MetadataOnly,
                        redaction_count: 0,
                        rules_version: "prototype-1".to_owned(),
                        policy_revision: 1,
                        truncated: false,
                    },
                },
                imported_at: NOW.to_owned(),
            }
        })
        .collect()
}
