use super::*;
use mochi_capture::{
    CaptureSanitizer, ClientSurface, CodexSessionSource, Spool, SpoolLimits,
    model::{
        NormalizedEvent, PendingEvent, Sensitivity, SensitivityClassification, SourceDescriptor,
        UserPromptPayload,
    },
    source::{Clock as CaptureClock, SessionSource},
};
use mochi_domain::{CodingSession, Project, ProjectId, UtcTimestamp};
use rusqlite::Connection;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;
use uuid::Uuid;

const NOW: &str = "2026-09-17T10:00:00Z";
const PROJECT_ID: &str = "10000000-0000-4000-8000-000000000001";

struct FixedClock;

struct FixedCaptureClock;

impl CaptureClock for FixedCaptureClock {
    fn now_rfc3339(&self) -> String {
        NOW.to_owned()
    }
}

impl Clock for FixedClock {
    fn now_rfc3339(&self) -> StorageResult<String> {
        Ok(NOW.to_owned())
    }
}

fn store(root: &TempDir) -> SqliteStore {
    SqliteStore::open(root.path().join("app/mochi.sqlite3"), Arc::new(FixedClock))
        .expect("test database opens")
}

fn project(root: &Path) -> Project {
    Project {
        id: ProjectId::parse(PROJECT_ID).expect("project id"),
        display_name: "Synthetic Project".to_owned(),
        root_path: root.to_string_lossy().into_owned(),
        repository_identity: None,
        created_at: UtcTimestamp::parse("2026-09-17T09:00:00Z").expect("created"),
        last_seen_at: UtcTimestamp::parse(NOW).expect("last seen"),
    }
}

fn create_project(store: &SqliteStore, root: &Path, revision: u64) {
    store
        .create_project(
            &project(root),
            CapturePolicy {
                tracking_enabled: true,
                revision,
            },
        )
        .expect("project is stored");
}

fn source() -> SourceDescriptor {
    SourceDescriptor {
        provider: "codex".to_owned(),
        adapter_version: "prototype-1".to_owned(),
        transport: "hooks".to_owned(),
        client_surface: ClientSurface::Cli,
    }
}

fn pending(source_event_id: &str, text: &str, revision: u64) -> PendingEvent {
    PendingEvent {
        source_event_id: Some(source_event_id.to_owned()),
        source_event_type: "UserPromptSubmit".to_owned(),
        external_session_id: Some("external-session".to_owned()),
        external_turn_id: Some("external-turn".to_owned()),
        external_tool_use_id: None,
        event: NormalizedEvent::UserPrompt(UserPromptPayload {
            text: text.to_owned(),
            turn_ref: Some("external-turn".to_owned()),
        }),
        sensitivity: Sensitivity {
            classification: SensitivityClassification::Sanitized,
            redaction_count: 0,
            rules_version: "prototype-1".to_owned(),
            policy_revision: revision,
            truncated: false,
        },
    }
}

fn spool(root: &TempDir) -> Spool {
    Spool::new(root.path().join("spool"), SpoolLimits::default())
}

fn fixture_session() -> CodingSession {
    CodingSession::from_json(include_str!(
        "../../../packages/domain/fixtures/coding-session-cli-complete.json"
    ))
    .expect("shared session fixture")
}

#[test]
fn opens_private_database_and_reopens_migrations_idempotently() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("app/mochi.sqlite3");
    let first = SqliteStore::open(&path, Arc::new(FixedClock)).expect("first open");
    assert_eq!(first.schema_version(), Ok(3));
    drop(first);
    let second = SqliteStore::open(&path, Arc::new(FixedClock)).expect("second open");
    assert_eq!(second.schema_version(), Ok(3));
    #[cfg(unix)]
    {
        assert_eq!(
            fs::metadata(path.parent().expect("parent"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        for suffix in ["-wal", "-shm"] {
            let sidecar = std::path::PathBuf::from(format!("{}{}", path.display(), suffix));
            if sidecar.exists() {
                assert_eq!(
                    fs::metadata(sidecar).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }
}

#[test]
fn rejects_changed_or_newer_migration_history() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("app/mochi.sqlite3");
    drop(SqliteStore::open(&path, Arc::new(FixedClock)).expect("open"));
    let connection = Connection::open(&path).expect("raw connection");
    connection
        .execute(
            "UPDATE schema_migrations SET checksum = 'changed' WHERE version = 1",
            [],
        )
        .expect("mutate checksum");
    drop(connection);
    assert!(matches!(
        SqliteStore::open(&path, Arc::new(FixedClock)),
        Err(StorageError::ChecksumMismatch)
    ));

    let newer_root = TempDir::new().expect("newer root");
    let newer_path = newer_root.path().join("app/mochi.sqlite3");
    drop(SqliteStore::open(&newer_path, Arc::new(FixedClock)).expect("open"));
    let connection = Connection::open(&newer_path).expect("raw connection");
    connection
        .execute(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at)
             VALUES (99, 'future', 'x', ?1)",
            [NOW],
        )
        .expect("future migration");
    drop(connection);
    assert!(matches!(
        SqliteStore::open(&newer_path, Arc::new(FixedClock)),
        Err(StorageError::SchemaTooNew)
    ));
}

#[test]
fn migration_two_preserves_populated_migration_one_data() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("app/mochi.sqlite3");
    let store = SqliteStore::open(&path, Arc::new(FixedClock)).expect("open");
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("event-1", "one", 1)],
        )
        .unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().inserted, 1);
    drop(store);
    let connection = Connection::open(&path).expect("raw connection");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE git_snapshots; DROP TABLE file_changes; DROP TABLE command_executions;
             DROP TABLE tool_executions; DROP TABLE session_events; DROP TABLE session_turns;
             DROP TABLE sessions; DELETE FROM schema_migrations WHERE version = 2;",
        )
        .expect("rewind to migration one");
    drop(connection);
    let reopened = SqliteStore::open(&path, Arc::new(FixedClock)).expect("migration two reapplies");
    assert_eq!(
        reopened
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn migration_three_upgrades_brief_three_data_without_rewriting_it() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("app/mochi.sqlite3");
    let store = SqliteStore::open(&path, Arc::new(FixedClock)).expect("open");
    create_project(&store, root.path(), 1);
    let session = fixture_session();
    store.insert_session(&session).expect("persist session");
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("brief-three-evidence", "preserved", 1)],
        )
        .unwrap();
    store.import_spool_batch(&spool, 100).unwrap();
    drop(store);

    let connection = Connection::open(&path).expect("raw connection");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = OFF;
             DROP TABLE ingress_assembly_state;
             DROP TABLE assembly_groups;
             DELETE FROM schema_migrations WHERE version = 3;",
        )
        .expect("rewind to the Brief 03 schema");
    drop(connection);

    let upgraded = SqliteStore::open(&path, Arc::new(FixedClock)).expect("migration applies");
    assert_eq!(upgraded.schema_version(), Ok(3));
    assert_eq!(
        upgraded
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        upgraded.get_session(session.data().id).unwrap(),
        Some(session)
    );
}

#[test]
fn ingress_import_is_idempotent_by_ingress_and_source_identity() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    let project_uuid = Uuid::parse_str(PROJECT_ID).unwrap();
    spool
        .append(
            project_uuid,
            source(),
            NOW,
            vec![pending("same-source", "same text", 1)],
        )
        .unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().inserted, 1);
    spool
        .append(
            project_uuid,
            source(),
            NOW,
            vec![pending("same-source", "changed text", 1)],
        )
        .unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().duplicate, 1);
    spool
        .append(
            project_uuid,
            source(),
            NOW,
            vec![pending("different-source", "same text", 1)],
        )
        .unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().inserted, 1);
    let page = store
        .page_ingress(project(root.path()).id, None, Some(100))
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].record.receive_sequence, 1);
    assert_eq!(page.items[1].record.receive_sequence, 3);
}

#[test]
fn missing_source_identity_falls_back_to_distinct_ingress_ids() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    let mut first = pending("unused", "same text", 1);
    first.source_event_id = None;
    let mut second = first.clone();
    second.external_turn_id = Some("another-turn".to_owned());
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![first, second],
        )
        .unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().inserted, 2);
}

#[test]
fn policy_is_rechecked_before_durable_content_write() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("stale", "must not persist", 2)],
        )
        .unwrap();
    let outcome = store.import_spool_batch(&spool, 100).unwrap();
    assert_eq!(outcome.rejected, 1);
    assert!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .is_empty()
    );
    let connection = store.lock().unwrap();
    let diagnostic: String = connection
        .query_row("SELECT reason FROM ingress_rejections", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(diagnostic, "policy_rejected");
}

#[test]
fn malformed_spool_is_recorded_without_raw_content() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    fs::create_dir_all(spool.root()).unwrap();
    let id = Uuid::new_v4();
    let path = spool.root().join(format!("{:020}-{id}.json", 1));
    let canary = "synthetic-secret-canary";
    fs::write(&path, format!("{{broken:{canary}")).unwrap();
    let outcome = store.import_spool_batch(&spool, 100).unwrap();
    assert_eq!(outcome.rejected, 1);
    assert!(!path.exists());
    drop(store);
    let database = fs::read(root.path().join("app/mochi.sqlite3")).unwrap();
    assert!(!String::from_utf8_lossy(&database).contains(canary));
}

#[test]
fn forged_valid_spool_secret_is_rejected_before_sqlite_content_write() {
    let root = TempDir::new().expect("root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    let mut record = spool
        .append(
            Uuid::parse_str(PROJECT_ID).expect("id"),
            source(),
            NOW,
            vec![pending("forged-event", "safe", 1)],
        )
        .expect("initial record")
        .remove(0);
    let canary = "sk-synthetic-forged-spool-value-123";
    record.event = NormalizedEvent::UserPrompt(UserPromptPayload {
        text: format!("OPENAI_API_KEY={canary}"),
        turn_ref: Some("external-turn".to_owned()),
    });
    let path = spool.root().join(format!(
        "{:020}-{}.json",
        record.receive_sequence, record.id
    ));
    fs::write(&path, serde_json::to_vec(&record).expect("forged json")).expect("forge record");
    let outcome = store.import_spool_batch(&spool, 100).expect("import");
    assert_eq!(outcome.rejected, 1);
    assert_eq!(outcome.inserted, 0);
    assert!(!path.exists());
    assert!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .expect("page")
            .items
            .is_empty()
    );
    let connection = Connection::open(root.path().join("app/mochi.sqlite3")).expect("connection");
    let reason: String = connection
        .query_row("SELECT reason FROM ingress_rejections LIMIT 1", [], |row| {
            row.get(0)
        })
        .expect("rejection");
    assert_eq!(reason, "redaction_rejected");
    drop(connection);
    drop(store);
    for entry in fs::read_dir(root.path().join("app")).expect("database files") {
        let bytes = fs::read(entry.expect("entry").path()).expect("bytes");
        assert!(
            !bytes
                .windows(canary.len())
                .any(|part| part == canary.as_bytes())
        );
    }
}

#[test]
fn command_output_canaries_are_absent_from_spool_and_sqlite() {
    let root = TempDir::new().expect("root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    let canaries = [
        "synthetic-command-db-pass",
        "synthetic-command-private-body",
    ];
    let capture = CodexSessionSource::new(
        Uuid::parse_str(PROJECT_ID).expect("id"),
        root.path().to_path_buf(),
        ClientSurface::Cli,
        1,
        CaptureSanitizer::new(root.path()).expect("redactor"),
        FixedCaptureClock,
    );
    let raw = serde_json::to_vec(&serde_json::json!({
        "hook_event_name": "PostToolUse",
        "session_id": "synthetic-session",
        "cwd": root.path(),
        "turn_id": "synthetic-turn",
        "tool_use_id": "synthetic-tool",
        "tool_name": "Bash",
        "tool_response": {
            "stdout": "DATABASE_URL=postgres://user:synthetic-command-db-pass@localhost/db\n-----BEGIN PRIVATE KEY-----\nsynthetic-command-private-body\n-----END PRIVATE KEY-----"
        }
    }))
    .expect("raw fixture");
    let pending = capture.normalize(&raw).expect("normalize");
    let normalized =
        serde_json::to_string(&pending.iter().map(|item| &item.event).collect::<Vec<_>>())
            .expect("normalized events");
    for canary in canaries {
        assert!(!normalized.contains(canary), "normalized command canary");
    }
    spool
        .append(capture.project_id(), capture.descriptor(), NOW, pending)
        .expect("spool");
    let spool_bytes =
        serde_json::to_string(&spool.read_records().expect("spool records")).expect("spool json");
    for canary in canaries {
        assert!(!spool_bytes.contains(canary), "spool command canary");
    }
    let outcome = store.import_spool_batch(&spool, 100).expect("import");
    assert_eq!(outcome.inserted, 2);
    assert_eq!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .expect("page")
            .items
            .len(),
        2
    );
    drop(store);
    for entry in fs::read_dir(root.path().join("app")).expect("database files") {
        let bytes = fs::read(entry.expect("entry").path()).expect("bytes");
        for canary in canaries {
            assert!(
                !bytes
                    .windows(canary.len())
                    .any(|part| part == canary.as_bytes()),
                "sqlite command canary"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn committed_import_replays_safely_when_acknowledgement_fails() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("event-1", "one", 1)],
        )
        .unwrap();
    fs::set_permissions(spool.root(), fs::Permissions::from_mode(0o500)).unwrap();
    let first = store.import_spool_batch(&spool, 100);
    fs::set_permissions(spool.root(), fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(first, Err(StorageError::Acknowledgement));
    assert_eq!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().duplicate, 1);
    assert!(spool.scan_batch(100).unwrap().is_empty());
}

#[test]
fn ingress_pagination_and_tombstones_are_bounded() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    assert_eq!(
        store.page_ingress(project(root.path()).id, None, Some(101)),
        Err(StorageError::InvalidInput)
    );
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    let records = spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("one", "same", 1), pending("two", "same", 1)],
        )
        .unwrap();
    store.import_spool_batch(&spool, 100).unwrap();
    let first = store
        .page_ingress(project(root.path()).id, None, Some(1))
        .unwrap();
    assert_eq!(first.items.len(), 1);
    let second = store
        .page_ingress(project(root.path()).id, first.next_cursor.as_ref(), Some(1))
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(store.delete_ingress(records[0].id).unwrap());
    let replay_path = spool.root().join(format!(
        "{:020}-{}.json",
        records[0].receive_sequence, records[0].id
    ));
    fs::write(&replay_path, serde_json::to_vec(&records[0]).unwrap()).unwrap();
    assert_eq!(store.import_spool_batch(&spool, 100).unwrap().tombstoned, 1);
    assert!(!replay_path.exists());
    assert_eq!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn explicit_retention_purge_tombstones_deleted_evidence() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("retained-until-cutoff", "safe", 1)],
        )
        .unwrap();
    store.import_spool_batch(&spool, 100).unwrap();
    assert_eq!(
        store.purge_ingress_before("2026-09-18T00:00:00Z").unwrap(),
        1
    );
    let connection = store.lock().unwrap();
    let tombstones: i64 = connection
        .query_row("SELECT COUNT(*) FROM ingress_tombstones", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(tombstones, 2);
}

#[test]
fn validated_session_round_trips_idempotently_and_cascades() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let session = fixture_session();
    store.insert_session(&session).expect("insert session");
    store.insert_session(&session).expect("idempotent insert");
    assert_eq!(
        store.get_session(session.data().id).unwrap(),
        Some(session.clone())
    );
    let page = store
        .list_sessions(project(root.path()).id, None, None)
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(store.delete_session(session.data().id).unwrap());
    let connection = store.lock().unwrap();
    for table in [
        "session_turns",
        "session_events",
        "tool_executions",
        "command_executions",
        "file_changes",
        "git_snapshots",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table} cascades");
    }
}

#[test]
fn session_repository_preserves_partial_interrupted_and_git_uncertainty() {
    let base: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/domain/fixtures/coding-session-cli-complete.json"
    ))
    .unwrap();
    let mut variants = Vec::new();

    let mut desktop = base.clone();
    desktop["source"]["clientSurface"] = "desktop".into();
    desktop["captureCapabilities"]["userPrompt"] = "unknown".into();
    desktop["captureCapabilities"]["interrupts"] = "unknown".into();
    desktop["captureCompleteness"]["prompt"] = "unknown".into();
    desktop["captureCompleteness"]["interrupts"] = "unknown".into();
    variants.push(desktop);

    let mut interrupted = base.clone();
    interrupted["status"] = "interrupted".into();
    interrupted["commandExecutions"][0]["status"] = "interrupted".into();
    interrupted["commandExecutions"][0]["completedAt"] = serde_json::Value::Null;
    interrupted["commandExecutions"][0]["exitCode"] = serde_json::Value::Null;
    variants.push(interrupted);

    let mut no_git = base.clone();
    no_git["gitContext"] = serde_json::json!({
        "availability": "unavailable",
        "reason": "not_repository"
    });
    no_git["captureCompleteness"]["git"] = "not_applicable".into();
    variants.push(no_git);

    let mut truncated = base;
    truncated["gitContext"]["after"]["truncated"] = true.into();
    truncated["gitContext"]["after"]["warnings"] = serde_json::json!(["synthetic aggregate limit"]);
    truncated["captureCompleteness"]["git"] = "partial".into();
    variants.push(truncated);

    for value in variants {
        let root = TempDir::new().expect("temp root");
        let store = store(&root);
        create_project(&store, root.path(), 1);
        let encoded = serde_json::to_string(&value).unwrap();
        let session = CodingSession::from_json(&encoded).expect("variant validates");
        store.insert_session(&session).expect("variant persists");
        assert_eq!(store.get_session(session.data().id).unwrap(), Some(session));
    }
}

#[test]
fn changed_session_identity_conflicts_and_corrupt_rows_fail_closed() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let session = fixture_session();
    store.insert_session(&session).unwrap();
    let mut changed: serde_json::Value = serde_json::from_str(&session.to_json().unwrap()).unwrap();
    changed["source"]["adapterVersion"] = serde_json::Value::String("different".to_owned());
    let changed = CodingSession::from_json(&serde_json::to_string(&changed).unwrap()).unwrap();
    assert_eq!(
        store.insert_session(&changed),
        Err(StorageError::Constraint)
    );
    store
        .lock()
        .unwrap()
        .execute("UPDATE session_events SET row_json = '{}'", [])
        .unwrap();
    assert_eq!(
        store.get_session(session.data().id),
        Err(StorageError::Corrupt)
    );
}

#[test]
fn concurrent_duplicate_import_has_one_effect() {
    let root = TempDir::new().expect("temp root");
    let store = Arc::new(store(&root));
    create_project(&store, root.path(), 1);
    let spool_root = TempDir::new().expect("spool root");
    let spool_path = spool_root.path().join("spool");
    Spool::new(spool_path.clone(), SpoolLimits::default())
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("event-1", "one", 1)],
        )
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        let spool_path = spool_path.clone();
        workers.push(thread::spawn(move || {
            barrier.wait();
            store.import_spool_batch(&Spool::new(spool_path, SpoolLimits::default()), 100)
        }));
    }
    for worker in workers {
        let result = worker.join().unwrap();
        assert!(result.is_ok() || result == Err(StorageError::Acknowledgement));
    }
    assert_eq!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .len(),
        1
    );
}

#[test]
fn independent_writer_contention_times_out_without_partial_change() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let competing = Connection::open(store.path()).expect("competing connection");
    competing
        .execute_batch("BEGIN IMMEDIATE")
        .expect("hold writer lock");
    assert_eq!(
        store.update_capture_policy(project(root.path()).id, false),
        Err(StorageError::Busy)
    );
    competing.execute_batch("ROLLBACK").expect("release lock");
    let policy = store
        .get_project(project(root.path()).id)
        .unwrap()
        .unwrap()
        .capture_policy;
    assert_eq!(
        policy,
        CapturePolicy {
            tracking_enabled: true,
            revision: 1
        }
    );
}

#[test]
fn project_deletion_revokes_policy_and_cascades_content() {
    let root = TempDir::new().expect("temp root");
    let store = store(&root);
    create_project(&store, root.path(), 1);
    let session = fixture_session();
    store.insert_session(&session).unwrap();
    let spool_root = TempDir::new().expect("spool root");
    let spool = spool(&spool_root);
    spool
        .append(
            Uuid::parse_str(PROJECT_ID).unwrap(),
            source(),
            NOW,
            vec![pending("event-1", "one", 1)],
        )
        .unwrap();
    store.import_spool_batch(&spool, 100).unwrap();
    assert!(store.soft_delete_project(project(root.path()).id).unwrap());
    let stored = store
        .get_project(project(root.path()).id)
        .unwrap()
        .expect("soft deleted policy row remains");
    assert!(!stored.capture_policy.tracking_enabled);
    assert_eq!(stored.capture_policy.revision, 2);
    assert!(stored.deleted_at.is_some());
    assert!(store.get_session(session.data().id).unwrap().is_none());
    assert!(
        store
            .page_ingress(project(root.path()).id, None, None)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn corrupt_database_is_rejected_without_replacement() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("app/mochi.sqlite3");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = b"not a sqlite database";
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        SqliteStore::open(&path, Arc::new(FixedClock)),
        Err(StorageError::Corrupt)
    ));
    assert_eq!(fs::read(path).unwrap(), bytes);
}
