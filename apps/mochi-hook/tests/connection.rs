use mochi_capture::{Spool, SpoolLimits};
use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn real_helper_connects_only_after_exact_approval_captures_silently_and_disconnects() {
    let temp = TempDir::new().expect("temp");
    let root = temp.path().canonicalize().expect("root");
    let project = root.join("project");
    std::fs::create_dir(&project).expect("project");
    let state = root.join("integration");
    let spool = root.join("spool");
    let project_id = Uuid::new_v4();
    let helper = env!("CARGO_BIN_EXE_mochi-hook");
    let arguments = [
        "connect",
        "--approved-root",
        project.to_str().expect("project"),
        "--integration-state-root",
        state.to_str().expect("state"),
        "--project-id",
        &project_id.to_string(),
        "--policy-revision",
        "1",
        "--spool-root",
        spool.to_str().expect("spool"),
    ];
    let rejected = Command::new(helper)
        .env_clear()
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .expect("preview");
    assert!(rejected.status.success());
    assert!(!project.join(".codex/hooks.json").exists());
    let mut child = Command::new(helper)
        .env_clear()
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("connect");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    let approval = loop {
        let mut line = String::new();
        assert!(output.read_line(&mut line).expect("line") > 0);
        if let Some((_, text)) = line.split_once("type: approve ") {
            break format!("approve {}\n", text.trim());
        }
    };
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(approval.as_bytes())
        .expect("approve");
    let mut remaining = String::new();
    output.read_to_string(&mut remaining).expect("result");
    assert!(child.wait().expect("wait").success());
    assert!(remaining.contains("Changed"));
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(project.join(".codex/hooks.json")).expect("config"))
            .expect("json");
    assert_eq!(config["hooks"].as_object().expect("events").len(), 12);
    for event in ["SessionStart", "UserPromptSubmit", "Stop", "SessionEnd"] {
        let mut child = Command::new(helper)
            .env_clear()
            .args([
                "capture",
                "--project-id",
                &project_id.to_string(),
                "--approved-root",
                project.to_str().expect("project"),
                "--client-surface",
                "cli",
                "--policy-revision",
                "1",
                "--spool-root",
                spool.to_str().expect("spool"),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("capture");
        let payload = json!({"hook_event_name":event,"cwd":project,"session_id":"synthetic-connection-test","turn_id":"synthetic-turn", "source":"startup","prompt":"Explain a synthetic sum function", "last_assistant_message":"Synthetic function inspected"});
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(&serde_json::to_vec(&payload).expect("json"))
            .expect("payload");
        let result = child.wait_with_output().expect("wait");
        assert!(result.status.success());
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }
    let records = Spool::new(spool, SpoolLimits::default())
        .read_records()
        .expect("records");
    assert!(records.len() >= 4);
    assert!(records.iter().all(|v| v.project_id == project_id));
    let mut child = Command::new(helper)
        .env_clear()
        .args([
            "disconnect",
            "--approved-root",
            project.to_str().expect("root"),
            "--integration-state-root",
            state.to_str().expect("state"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("disconnect");
    let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).expect("line") > 0);
        if let Some((_, text)) = line.split_once("type: approve ") {
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(format!("approve {}\n", text.trim()).as_bytes())
                .expect("approve");
            break;
        }
    }
    let mut result = String::new();
    reader.read_to_string(&mut result).expect("result");
    assert!(child.wait().expect("wait").success());
    assert!(!project.join(".codex/hooks.json").exists());
}
