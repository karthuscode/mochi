use crate::git::{
    GitChange, GitCliContextReader, GitContentOmission, GitContextReader, SnapshotRole,
};
use crate::model::{
    CaptureGapReason, ClientSurface, CompletionStatus, NormalizedEvent, PendingEvent, Sensitivity,
    SensitivityClassification, UserPromptPayload,
};
use crate::sanitize::{ContentSanitizer, SanitizeError, SanitizedText};
use crate::source::{Clock, CodexSessionSource, IntakeError, SessionSource};
use crate::spool::{Spool, SpoolCandidateData, SpoolLimits, SpoolRejectionReason};
use crate::PrototypeSanitizer;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

const NOW: &str = "2026-09-16T08:00:00Z";

struct FixedClock;

impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        NOW.to_owned()
    }
}

struct FailingSanitizer;

impl ContentSanitizer for FailingSanitizer {
    fn sanitize_text(&self, _value: &str) -> Result<SanitizedText, SanitizeError> {
        Err(SanitizeError::UnsafeInput)
    }
}

fn source(root: &Path) -> CodexSessionSource<PrototypeSanitizer, FixedClock> {
    CodexSessionSource::new(
        Uuid::nil(),
        root.to_path_buf(),
        ClientSurface::Cli,
        1,
        PrototypeSanitizer::new(root).expect("test sanitizer compiles"),
        FixedClock,
    )
}

fn payload(root: &Path, event: &str, additions: Value) -> Vec<u8> {
    let mut value = json!({
        "hook_event_name": event,
        "session_id": "session-1",
        "cwd": root,
        "unknown_future_field": {"safe": true}
    });
    value
        .as_object_mut()
        .expect("fixture object")
        .extend(additions.as_object().expect("fixture additions").clone());
    serde_json::to_vec(&value).expect("fixture serializes")
}

fn event_name(event: &NormalizedEvent) -> &'static str {
    match event {
        NormalizedEvent::SessionStarted(_) => "session.started",
        NormalizedEvent::UserPrompt(_) => "user.prompt",
        NormalizedEvent::AgentMessage(_) => "agent.message",
        NormalizedEvent::ToolStarted(_) => "tool.started",
        NormalizedEvent::ToolCompleted(_) => "tool.completed",
        NormalizedEvent::PermissionRequested(_) => "permission.requested",
        NormalizedEvent::CommandExecuted(_) => "command.executed",
        NormalizedEvent::CommandResult(_) => "command.result",
        NormalizedEvent::TurnCompleted(_) => "turn.completed",
        NormalizedEvent::SessionStopped(_) => "session.stopped",
        NormalizedEvent::ContextCompacted(_) => "context.compacted",
        NormalizedEvent::CaptureGap(_) => "capture.gap",
    }
}

#[test]
fn parses_observed_payloads_and_tolerates_unknown_fields() {
    let root = TempDir::new().expect("temp root");
    let source = source(root.path());
    let events = source
        .normalize(&payload(
            root.path(),
            "SessionStart",
            json!({"source": "startup", "model": "synthetic"}),
        ))
        .expect("valid start");
    assert_eq!(events.len(), 1);
    assert_eq!(event_name(&events[0].event), "session.started");
}

#[test]
fn rejects_malformed_missing_and_oversized_payloads_without_panicking() {
    let root = TempDir::new().expect("temp root");
    let source = source(root.path());
    assert!(matches!(source.normalize(b""), Err(IntakeError::Malformed)));
    assert!(matches!(
        source.normalize(b"{"),
        Err(IntakeError::Malformed)
    ));
    assert!(matches!(
        source.normalize(&vec![b'x'; 1024 * 1024 + 1]),
        Err(IntakeError::Oversized)
    ));
    let missing = json!({"hook_event_name":"SessionStart","cwd":root.path()});
    assert!(matches!(
        source.normalize(&serde_json::to_vec(&missing).expect("fixture")),
        Err(IntakeError::MissingRequired(_))
    ));
    let missing_tool = payload(
        root.path(),
        "PreToolUse",
        json!({"turn_id":"turn-1","tool_name":"Bash","tool_input":{"command":"true"}}),
    );
    assert!(matches!(
        source.normalize(&missing_tool),
        Err(IntakeError::MissingRequired(_))
    ));
}

#[test]
fn sanitizes_before_the_first_spool_write() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let fake_secret = "sk-test_12345678901234567890";
    let events = source
        .normalize(&payload(
            root.path(),
            "UserPromptSubmit",
            json!({"turn_id":"turn-1","prompt":format!("Authorization: Bearer abcdefghijklmnop API_KEY={fake_secret}")}),
        ))
        .expect("normalizes");
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    spool
        .append(source.project_id(), source.descriptor(), NOW, events)
        .expect("spools");
    let durable = fs::read_dir(spool_root.path())
        .expect("read spool")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "json")
        })
        .flat_map(|entry| fs::read(entry.path()).unwrap_or_default())
        .collect::<Vec<_>>();
    let durable = String::from_utf8(durable).expect("json utf8");
    assert!(!durable.contains(fake_secret));
    assert!(!durable.contains("abcdefghijklmnop"));
    assert!(durable.contains("[REDACTED"));
}

#[test]
fn sanitizer_failure_writes_metadata_only_gap() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = CodexSessionSource::new(
        Uuid::nil(),
        root.path().to_path_buf(),
        ClientSurface::Cli,
        1,
        FailingSanitizer,
        FixedClock,
    );
    let fake_secret = "sk-test_12345678901234567890";
    let error = source
        .normalize(&payload(
            root.path(),
            "UserPromptSubmit",
            json!({"turn_id":"turn-1","prompt":fake_secret}),
        ))
        .expect_err("sanitizer fails");
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    let records = spool
        .append(
            source.project_id(),
            source.descriptor(),
            NOW,
            vec![source.metadata_gap(&error)],
        )
        .expect("metadata gap spools");
    assert_eq!(
        records[0].sensitivity.classification,
        SensitivityClassification::MetadataOnly
    );
    assert_eq!(event_name(&records[0].event), "capture.gap");
    assert!(!serde_json::to_string(&records)
        .expect("records serialize")
        .contains(fake_secret));
}

#[test]
fn duplicates_retain_a_stable_dedupe_key_and_distinct_ingress_ids() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let raw = payload(
        root.path(),
        "PreToolUse",
        json!({"turn_id":"turn-1","tool_use_id":"tool-1","tool_name":"Bash","tool_input":{"command":"true"}}),
    );
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    let first = spool
        .append(
            source.project_id(),
            source.descriptor(),
            NOW,
            source.normalize(&raw).expect("first normalize"),
        )
        .expect("first append");
    let second = spool
        .append(
            source.project_id(),
            source.descriptor(),
            NOW,
            source.normalize(&raw).expect("second normalize"),
        )
        .expect("second append");
    assert_eq!(first[0].source_event_id, second[0].source_event_id);
    assert_ne!(first[0].id, second[0].id);
    assert!(second[0].receive_sequence > first[0].receive_sequence);
}

#[test]
fn tool_pairs_share_the_external_correlation_id() {
    let root = TempDir::new().expect("temp root");
    let source = source(root.path());
    let start = source
        .normalize(&payload(
            root.path(),
            "PreToolUse",
            json!({"turn_id":"turn-1","tool_use_id":"tool-1","tool_name":"apply_patch","tool_input":{}}),
        ))
        .expect("start");
    let completed = source
        .normalize(&payload(
            root.path(),
            "PostToolUse",
            json!({"turn_id":"turn-1","tool_use_id":"tool-1","tool_name":"apply_patch","tool_input":{},"tool_response":"Done"}),
        ))
        .expect("complete");
    assert_eq!(start[0].external_tool_use_id.as_deref(), Some("tool-1"));
    assert_eq!(completed[0].external_tool_use_id.as_deref(), Some("tool-1"));
}

#[test]
fn denied_file_tool_response_is_metadata_only_before_spooling() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let raw = payload(
        root.path(),
        "PostToolUse",
        json!({
            "turn_id":"turn-1", "tool_use_id":"tool-1", "tool_name":"apply_patch",
            "tool_input":{"patch":"*** Begin Patch\n*** Update File: .env.local\n+SYNTHETIC_CANARY=visible-value\n*** End Patch"},
            "tool_response":{"path":".env.local","patch":"SYNTHETIC_CANARY=visible-value"}
        }),
    );
    let events = source.normalize(&raw).expect("normalizes");
    assert!(
        matches!(&events[0].event, NormalizedEvent::ToolCompleted(value) if value.summary.is_none())
    );
    Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default())
        .append(source.project_id(), source.descriptor(), NOW, events)
        .expect("spools");
    for entry in fs::read_dir(spool_root.path()).expect("read spool") {
        let bytes = fs::read(entry.expect("entry").path()).expect("spool bytes");
        assert!(!bytes
            .windows(b"visible-value".len())
            .any(|slice| slice == b"visible-value"));
        assert!(!bytes
            .windows(b".env.local".len())
            .any(|slice| slice == b".env.local"));
    }
}

#[test]
fn response_discovered_denied_path_overrides_allowed_input_path() {
    let root = TempDir::new().expect("root");
    let source = source(root.path());
    let raw = payload(
        root.path(),
        "PostToolUse",
        json!({
            "turn_id":"turn-1", "tool_use_id":"tool-1", "tool_name":"ReadFile",
            "tool_input":{"path":"src/main.ts"},
            "tool_response":{"path":".env", "content":"synthetic-sensitive-content"}
        }),
    );
    let events = source.normalize(&raw).expect("normalizes");
    assert!(
        matches!(&events[0].event, NormalizedEvent::ToolCompleted(value) if value.summary.is_none())
    );
}

#[test]
fn file_tool_response_requires_existing_bounded_allowed_file() {
    let root = TempDir::new().expect("root");
    fs::create_dir(root.path().join("src")).expect("src");
    fs::write(root.path().join("src/small.ts"), "export const ok = true;").expect("small");
    fs::write(root.path().join("src/large.ts"), vec![b'x'; 64 * 1024 + 1]).expect("large");
    let source = source(root.path());
    for (path, retained) in [
        ("src/small.ts", true),
        ("src/large.ts", false),
        ("src/missing.ts", false),
    ] {
        let raw = payload(
            root.path(),
            "PostToolUse",
            json!({
                "turn_id":"turn-1", "tool_use_id":"tool-1", "tool_name":"ReadFile",
                "tool_input":{"path":path}, "tool_response":{"content":"synthetic-content"}
            }),
        );
        let events = source.normalize(&raw).expect("normalizes");
        assert!(
            matches!(&events[0].event, NormalizedEvent::ToolCompleted(value) if value.summary.is_some() == retained),
            "{path}"
        );
    }
}

#[test]
fn git_policy_omits_user_exclusions_and_lockfile_content() {
    let repo = TempDir::new().expect("repo");
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "probe@example.invalid"],
    );
    git(repo.path(), &["config", "user.name", "Mochi Probe"]);
    fs::create_dir(repo.path().join("private")).expect("private dir");
    fs::write(repo.path().join(".mochiignore"), "private/\n").expect("ignore");
    fs::write(
        repo.path().join("private/notes.ts"),
        "synthetic-hidden-content",
    )
    .expect("private file");
    fs::write(repo.path().join("pnpm-lock.yaml"), "synthetic-lock-content").expect("lockfile");
    fs::write(repo.path().join("package.json"), "{\"name\":\"synthetic\"}").expect("manifest");
    git(repo.path(), &["add", "."]);
    let snapshot = GitCliContextReader
        .snapshot(Uuid::new_v4(), repo.path(), SnapshotRole::Baseline)
        .expect("snapshot");
    let serialized = serde_json::to_string(&snapshot).expect("json");
    assert!(!serialized.contains("synthetic-hidden-content"));
    assert!(!serialized.contains("private/notes.ts"));
    assert!(!serialized.contains("synthetic-lock-content"));
    assert!(snapshot
        .files
        .iter()
        .any(|file| file.path == "pnpm-lock.yaml"
            && file.omission == Some(GitContentOmission::Policy)));
    assert!(snapshot
        .files
        .iter()
        .any(|file| file.path == "package.json" && file.content.is_some()));
}

#[cfg(unix)]
#[test]
fn git_snapshot_never_reads_symlinked_outside_content() {
    use std::os::unix::fs::symlink;
    let repo = TempDir::new().expect("repo");
    let outside = TempDir::new().expect("outside");
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "probe@example.invalid"],
    );
    git(repo.path(), &["config", "user.name", "Mochi Probe"]);
    fs::write(
        outside.path().join("outside.ts"),
        "synthetic-outside-content",
    )
    .expect("outside file");
    symlink(
        outside.path().join("outside.ts"),
        repo.path().join("linked.ts"),
    )
    .expect("link");
    git(repo.path(), &["add", "."]);
    let snapshot = GitCliContextReader
        .snapshot(Uuid::new_v4(), repo.path(), SnapshotRole::Baseline)
        .expect("snapshot");
    let serialized = serde_json::to_string(&snapshot).expect("json");
    assert!(!serialized.contains("synthetic-outside-content"));
    assert!(!serialized.contains("linked.ts"));
    assert!(snapshot.excluded_count >= 1);
}

#[test]
fn synthetic_canaries_are_absent_from_normalized_events_and_spool() {
    let root = TempDir::new().expect("root");
    let spool_root = TempDir::new().expect("spool");
    fs::create_dir(root.path().join("src")).expect("src");
    fs::write(root.path().join("src/main.ts"), "export const x = 1;").expect("file");
    let source = source(root.path());
    let fixtures = [
        payload(
            root.path(),
            "UserPromptSubmit",
            json!({"turn_id":"turn-1", "prompt":"Use OPENAI_API_KEY=sk-synthetic-example-value-123"}),
        ),
        payload(
            root.path(),
            "PreToolUse",
            json!({"turn_id":"turn-1", "tool_use_id":"tool-1", "tool_name":"Bash", "tool_input":{"command":"curl -H 'Authorization: Bearer synthetic-token-value' https://example.invalid"}}),
        ),
        payload(
            root.path(),
            "PostToolUse",
            json!({"turn_id":"turn-1", "tool_use_id":"tool-1", "tool_name":"Bash", "tool_response":{"stdout":"DATABASE_URL=postgres://user:synthetic-db-pass@localhost/db\n-----BEGIN PRIVATE KEY-----\nsynthetic-private-body\n-----END PRIVATE KEY-----"}}),
        ),
        payload(
            root.path(),
            "PostToolUse",
            json!({"turn_id":"turn-1", "tool_use_id":"tool-2", "tool_name":"ReadFile", "tool_input":{"path":"src/main.ts"}, "tool_response":{"client_secret":"synthetic-client-value", "ordinary":"safe"}}),
        ),
        payload(
            root.path(),
            "Stop",
            json!({"turn_id":"turn-1", "last_assistant_message":"Done with ghp_SyntheticTokenValue123456789"}),
        ),
    ];
    let canaries = [
        "sk-synthetic-example-value-123",
        "synthetic-token-value",
        "synthetic-db-pass",
        "synthetic-private-body",
        "synthetic-client-value",
        "ghp_SyntheticTokenValue123456789",
    ];
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    for raw in fixtures {
        let pending = source.normalize(&raw).expect("normalize");
        let normalized = pending
            .iter()
            .map(|item| serde_json::to_string(&item.event).expect("normalized"))
            .collect::<String>();
        for canary in canaries {
            assert!(!normalized.contains(canary), "normalized canary");
        }
        spool
            .append(source.project_id(), source.descriptor(), NOW, pending)
            .expect("spool");
    }
    let durable = fs::read_dir(spool_root.path())
        .expect("entries")
        .flat_map(|entry| fs::read(entry.expect("entry").path()).expect("read"))
        .collect::<Vec<_>>();
    let durable = String::from_utf8_lossy(&durable);
    for canary in canaries {
        assert!(!durable.contains(canary), "spool canary");
    }
    assert!(durable.contains("[REDACTED:OPENAI_API_KEY]"));
    assert!(durable.contains("[REDACTED:BEARER_TOKEN]"));
    assert!(durable.contains("[REDACTED:PRIVATE_KEY]"));
    assert!(durable.contains("[REDACTED:GENERIC_SECRET]"));
}

#[test]
fn unterminated_private_key_drops_prompt_content_before_spooling() {
    let root = TempDir::new().expect("root");
    let spool_root = TempDir::new().expect("spool");
    let source = source(root.path());
    let raw = payload(
        root.path(),
        "UserPromptSubmit",
        json!({
            "turn_id":"turn-1", "prompt":"-----BEGIN PRIVATE KEY-----\nsynthetic-unclosed-body"
        }),
    );
    let error = source.normalize(&raw).expect_err("reject unsafe block");
    assert_eq!(error, IntakeError::SanitizationFailed);
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    let stored = spool
        .append(
            source.project_id(),
            source.descriptor(),
            NOW,
            vec![source.metadata_gap(&error)],
        )
        .expect("gap");
    assert!(
        matches!(stored[0].event, NormalizedEvent::CaptureGap(ref gap) if gap.reason == CaptureGapReason::SanitizationFailed)
    );
    let durable = serde_json::to_string(&spool.read_records().expect("records")).expect("json");
    assert!(!durable.contains("synthetic-unclosed-body"));
}

#[test]
fn spool_rejects_unsanitized_direct_caller_content() {
    let spool_root = TempDir::new().expect("spool");
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    let pending = PendingEvent {
        source_event_id: None,
        source_event_type: "UserPromptSubmit".to_owned(),
        external_session_id: Some("session-1".to_owned()),
        external_turn_id: None,
        external_tool_use_id: None,
        event: NormalizedEvent::UserPrompt(UserPromptPayload {
            text: "OPENAI_API_KEY=sk-synthetic-direct-spool-123".to_owned(),
            turn_ref: None,
        }),
        sensitivity: Sensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: 0,
            rules_version: "forged".to_owned(),
            policy_revision: 1,
            truncated: false,
        },
    };
    let stored = spool
        .append(
            Uuid::nil(),
            source(TempDir::new().expect("root").path()).descriptor(),
            NOW,
            vec![pending],
        )
        .expect("append");
    assert!(
        matches!(stored[0].event, NormalizedEvent::CaptureGap(ref gap) if gap.reason == CaptureGapReason::SanitizationFailed)
    );
    let records = spool.read_records().expect("records");
    assert!(!serde_json::to_string(&records)
        .expect("json")
        .contains("sk-synthetic-direct-spool-123"));
}

#[test]
fn git_allowed_source_content_uses_shared_redaction() {
    let repo = TempDir::new().expect("repo");
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "probe@example.invalid"],
    );
    git(repo.path(), &["config", "user.name", "Mochi Probe"]);
    fs::create_dir(repo.path().join("src")).expect("src");
    fs::write(
        repo.path().join("src/main.ts"),
        "const token = 'ghp_SyntheticTokenValue123456789';\n",
    )
    .expect("file");
    fs::write(
        repo.path().join("sk-synthetic-example-value-123.ts"),
        "safe\n",
    )
    .expect("secret path");
    git(repo.path(), &["add", "."]);
    git(
        repo.path(),
        &["checkout", "-qb", "sk-synthetic-branch-value-123"],
    );
    let snapshot = GitCliContextReader
        .snapshot(Uuid::new_v4(), repo.path(), SnapshotRole::Baseline)
        .expect("snapshot");
    let serialized = serde_json::to_string(&snapshot).expect("json");
    assert!(!serialized.contains("ghp_SyntheticTokenValue123456789"));
    assert!(serialized.contains("[REDACTED:GITHUB_TOKEN]"));
    assert!(!serialized.contains("sk-synthetic-example-value-123"));
    assert_eq!(snapshot.branch, None);
}

#[test]
fn interrupted_tool_remains_without_a_synthetic_completion() {
    let root = TempDir::new().expect("temp root");
    let source = source(root.path());
    let mut events = source
        .normalize(&payload(
            root.path(),
            "PreToolUse",
            json!({"turn_id":"turn-1","tool_use_id":"tool-1","tool_name":"Bash","tool_input":{"command":"sleep 30"}}),
        ))
        .expect("start");
    events.extend(
        source
            .normalize(&payload(
                root.path(),
                "Interrupt",
                json!({"turn_id":"turn-1"}),
            ))
            .expect("interrupt"),
    );
    assert!(events
        .iter()
        .any(|event| matches!(event.event, NormalizedEvent::ToolStarted(_))));
    assert!(!events
        .iter()
        .any(|event| matches!(event.event, NormalizedEvent::ToolCompleted(_))));
    assert!(events.iter().any(|event| matches!(
        event.event,
        NormalizedEvent::TurnCompleted(ref payload) if payload.status == CompletionStatus::Cancelled
    )));
}

#[test]
fn receive_order_and_session_partition_survive_missing_end() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    for session in ["session-a", "session-b"] {
        let raw = payload(
            root.path(),
            "SessionStart",
            json!({"session_id":session,"source":"startup"}),
        );
        spool
            .append(
                source.project_id(),
                source.descriptor(),
                NOW,
                source.normalize(&raw).expect("normalizes"),
            )
            .expect("spools");
    }
    let records = spool.read_records().expect("reads records");
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].receive_sequence, 1);
    assert_eq!(records[1].receive_sequence, 2);
    assert_ne!(
        records[0].external_session_id,
        records[1].external_session_id
    );
    assert!(!records
        .iter()
        .any(|record| matches!(record.event, NormalizedEvent::SessionStopped(_))));
}

#[test]
fn spool_bounds_evict_oldest_with_metadata() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let spool = Spool::new(
        spool_root.path().to_path_buf(),
        SpoolLimits {
            max_event_bytes: 64 * 1024,
            max_spool_bytes: 900,
            max_age: Duration::from_secs(7 * 24 * 60 * 60),
        },
    );
    for turn in ["turn-1", "turn-2", "turn-3"] {
        let raw = payload(
            root.path(),
            "UserPromptSubmit",
            json!({"turn_id":turn,"prompt":"bounded payload"}),
        );
        spool
            .append(
                source.project_id(),
                source.descriptor(),
                NOW,
                source.normalize(&raw).expect("normalizes"),
            )
            .expect("spools");
    }
    assert!(spool.read_records().expect("records").len() < 3);
    assert!(spool.read_state().expect("state").evicted_count > 0);
}

#[test]
fn concurrent_spool_writers_keep_every_event_and_unique_order() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let barrier = Arc::new(Barrier::new(20));
    let mut writers = Vec::new();
    for index in 0..20 {
        let approved_root = root.path().to_path_buf();
        let spool_root = spool_root.path().to_path_buf();
        let barrier = Arc::clone(&barrier);
        writers.push(thread::spawn(move || {
            let source = source(&approved_root);
            let raw = payload(
                &approved_root,
                "UserPromptSubmit",
                json!({"turn_id":format!("turn-{index}"),"prompt":"burst"}),
            );
            barrier.wait();
            Spool::new(spool_root, SpoolLimits::default())
                .append(
                    source.project_id(),
                    source.descriptor(),
                    NOW,
                    source.normalize(&raw).expect("normalizes"),
                )
                .expect("concurrent append");
        }));
    }
    for writer in writers {
        writer.join().expect("writer thread");
    }

    let records = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default())
        .read_records()
        .expect("records");
    let sequences = records
        .iter()
        .map(|record| record.receive_sequence)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 20);
    assert_eq!(sequences, (1..=20).collect::<Vec<_>>());
}

#[test]
fn bounded_scan_reports_malformed_entries_and_acknowledges_exact_tokens() {
    let spool_root = TempDir::new().expect("temp spool");
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    fs::create_dir_all(spool.root()).expect("spool directory");
    let id = Uuid::new_v4();
    let path = spool.root().join(format!("{:020}-{id}.json", 7));
    fs::write(&path, b"{malformed").expect("malformed fixture");

    let candidates = spool.scan_batch(500).expect("bounded scan");
    assert_eq!(candidates.len(), 1);
    assert!(matches!(
        candidates[0].data(),
        SpoolCandidateData::Rejected {
            reason: SpoolRejectionReason::Malformed,
            receive_sequence: Some(7)
        }
    ));
    assert_eq!(candidates[0].token().opaque_id().len(), 64);
    spool
        .acknowledge(&[candidates[0].token().clone()])
        .expect("exact token acknowledges");
    assert!(!path.exists());
}

#[test]
fn invalid_event_filename_can_only_be_removed_through_its_opaque_scan_token() {
    let spool_root = TempDir::new().expect("temp spool");
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    fs::create_dir_all(spool.root()).expect("spool directory");
    let path = spool.root().join("unexpected.json");
    fs::write(&path, b"{}").expect("invalid filename fixture");
    let candidates = spool.scan_batch(1).expect("scan");
    assert!(matches!(
        candidates[0].data(),
        SpoolCandidateData::Rejected {
            reason: SpoolRejectionReason::InvalidFilename,
            receive_sequence: None
        }
    ));
    spool
        .acknowledge(&[candidates[0].token().clone()])
        .expect("opaque token removes its exact direct child");
    assert!(!path.exists());
}

#[test]
fn acknowledgement_tokens_are_bound_to_their_spool_root() {
    let approved_root = TempDir::new().expect("approved root");
    let first_root = TempDir::new().expect("first spool");
    let second_root = TempDir::new().expect("second spool");
    let source = source(approved_root.path());
    let first = Spool::new(first_root.path().to_path_buf(), SpoolLimits::default());
    first
        .append(
            source.project_id(),
            source.descriptor(),
            NOW,
            vec![PendingEvent {
                source_event_id: Some("root-bound".to_owned()),
                source_event_type: "UserPromptSubmit".to_owned(),
                external_session_id: Some("session-1".to_owned()),
                external_turn_id: Some("turn-1".to_owned()),
                external_tool_use_id: None,
                event: NormalizedEvent::UserPrompt(UserPromptPayload {
                    text: "safe".to_owned(),
                    turn_ref: Some("turn-1".to_owned()),
                }),
                sensitivity: Sensitivity {
                    classification: SensitivityClassification::Sanitized,
                    redaction_count: 0,
                    rules_version: "prototype-1".to_owned(),
                    policy_revision: 1,
                    truncated: false,
                },
            }],
        )
        .expect("append");
    let token = first.scan_batch(1).unwrap()[0].token().clone();
    let second = Spool::new(second_root.path().to_path_buf(), SpoolLimits::default());
    assert!(second.acknowledge(&[token]).is_err());
    assert_eq!(first.scan_batch(1).unwrap().len(), 1);
}

#[test]
fn oversized_normalized_event_becomes_a_metadata_only_gap() {
    let root = TempDir::new().expect("temp root");
    let spool_root = TempDir::new().expect("temp spool");
    let source = source(root.path());
    let spool = Spool::new(spool_root.path().to_path_buf(), SpoolLimits::default());
    let event = PendingEvent {
        source_event_id: Some("oversized-event".to_owned()),
        source_event_type: "UserPromptSubmit".to_owned(),
        external_session_id: Some("session-1".to_owned()),
        external_turn_id: Some("turn-1".to_owned()),
        external_tool_use_id: None,
        event: NormalizedEvent::UserPrompt(UserPromptPayload {
            text: "x".repeat(70 * 1024),
            turn_ref: Some("turn-1".to_owned()),
        }),
        sensitivity: Sensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: 0,
            rules_version: "prototype-1".to_owned(),
            policy_revision: 1,
            truncated: false,
        },
    };

    let records = spool
        .append(source.project_id(), source.descriptor(), NOW, vec![event])
        .expect("oversized event falls back");

    assert!(matches!(
        records[0].event,
        NormalizedEvent::CaptureGap(ref payload)
            if payload.reason == CaptureGapReason::Overflow
    ));
    assert_eq!(
        records[0].sensitivity.classification,
        SensitivityClassification::MetadataOnly
    );
    assert!(records[0].sensitivity.truncated);
    let durable_event = fs::read_dir(spool_root.path())
        .expect("spool directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|value| value == "json"))
        .expect("event file");
    assert!(fs::metadata(durable_event).expect("event metadata").len() <= 64 * 1024);
}

#[test]
fn git_snapshots_preserve_dirty_baseline_and_bound_sensitive_large_content() {
    let repo = TempDir::new().expect("temp repo");
    git(repo.path(), &["init", "-q"]);
    git(
        repo.path(),
        &["config", "user.email", "probe@example.invalid"],
    );
    git(repo.path(), &["config", "user.name", "Mochi Probe"]);
    fs::write(repo.path().join("preexisting.txt"), "baseline\n").expect("write baseline");
    fs::write(repo.path().join("code.rs"), "fn value() -> i32 { 1 }\n").expect("write code");
    fs::write(repo.path().join("deleted.txt"), "delete me\n").expect("write deleted");
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "baseline"]);
    fs::write(
        repo.path().join("preexisting.txt"),
        "baseline\ndirty before\n",
    )
    .expect("dirty baseline");
    fs::write(repo.path().join(".env"), "TOKEN=synthetic-secret\n").expect("secret fixture");
    fs::create_dir(repo.path().join(".codex")).expect("codex directory");
    fs::write(
        repo.path().join(".codex/hooks.json"),
        "{\"api_key\":\"sk-test_should-never-be-snapshotted\"}\n",
    )
    .expect("codex fixture");

    let reader = GitCliContextReader;
    let project_id = Uuid::new_v4();
    let baseline = reader
        .snapshot(project_id, repo.path(), SnapshotRole::Baseline)
        .expect("baseline snapshot");
    let preexisting_baseline = baseline
        .files
        .iter()
        .find(|file| file.path == "preexisting.txt")
        .and_then(|file| file.content_hash.clone())
        .expect("baseline hash");
    assert!(baseline.excluded_count >= 1);
    assert!(!serde_json::to_string(&baseline)
        .expect("serialize")
        .contains("synthetic-secret"));
    assert!(!baseline
        .files
        .iter()
        .any(|file| file.path.starts_with(".codex/")));

    fs::write(repo.path().join("code.rs"), "fn value() -> i32 { 2 }\n").expect("modify code");
    fs::remove_file(repo.path().join("deleted.txt")).expect("delete file");
    git(repo.path(), &["mv", "preexisting.txt", "renamed.txt"]);
    fs::write(repo.path().join("large.txt"), vec![b'x'; 64 * 1024 + 1]).expect("large file");
    let final_snapshot = reader
        .snapshot(project_id, repo.path(), SnapshotRole::Final)
        .expect("final snapshot");
    assert!(final_snapshot
        .files
        .iter()
        .any(|file| file.change == GitChange::Deleted));
    assert!(final_snapshot.files.iter().any(|file| {
        file.change == GitChange::Renamed && file.old_path.as_deref() == Some("preexisting.txt")
    }));
    assert!(final_snapshot.files.iter().any(|file| {
        file.path == "large.txt" && file.omission == Some(GitContentOmission::SizeLimit)
    }));
    let renamed_hash = final_snapshot
        .files
        .iter()
        .find(|file| file.path == "renamed.txt")
        .and_then(|file| file.content_hash.clone())
        .expect("renamed hash");
    assert_eq!(preexisting_baseline, renamed_hash);
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git command starts");
    assert!(status.success(), "git command failed: {args:?}");
}
