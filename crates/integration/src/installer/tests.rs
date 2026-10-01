use super::*;
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    project: PathBuf,
    installer: CodexInstaller,
    request: InstallRequest,
}
fn fixture() -> Fixture {
    let temp = TempDir::new().expect("temp");
    let root = temp.path().canonicalize().expect("root");
    let project = root.join("project with ' spaces");
    fs::create_dir(&project).expect("project");
    let helper = root.join("mochi-hook");
    fs::write(&helper, "#!/bin/sh\necho 'mochi-hook 0.1.0'\n").expect("helper");
    #[cfg(unix)]
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).expect("mode");
    let request = InstallRequest {
        project_id: Uuid::new_v4(),
        approved_root: project.clone(),
        helper_path: helper,
        spool_root: root.join("spool"),
        policy_revision: 1,
    };
    let installer = CodexInstaller::new(root.join("installer")).expect("installer");
    Fixture {
        _temp: temp,
        root,
        project,
        installer,
        request,
    }
}
fn config(f: &Fixture) -> PathBuf {
    f.project.join(".codex/hooks.json")
}
fn write_config(f: &Fixture, text: &str) {
    fs::create_dir_all(f.project.join(".codex")).expect("codex");
    fs::write(config(f), text).expect("config");
}
fn install(f: &Fixture) {
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("plan");
    let id = plan.preview().plan_id;
    assert_eq!(f.installer.apply(plan, id), Ok(InstallOutcome::Changed));
}

#[test]
fn preview_and_wrong_approval_do_not_edit_configuration() {
    let f = fixture();
    write_config(&f, "{\"description\":\"keep exactly\"}");
    let original = fs::read(config(&f)).expect("bytes");
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("plan");
    assert_eq!(plan.preview().events.len(), 12);
    assert!(plan.preview().requires_codex_trust_review);
    assert!(!serde_json::to_string(plan.preview())
        .expect("preview")
        .contains("keep exactly"));
    assert_eq!(
        f.installer.apply(plan, Uuid::new_v4()),
        Err(InstallError::ApprovalRequired)
    );
    assert_eq!(fs::read(config(&f)).expect("after"), original);
}

#[test]
fn install_is_idempotent_and_disconnect_preserves_unrelated_hooks_and_bytes() {
    let f = fixture();
    let original = "{\n  \"description\" : \"exact whitespace stays\",\n  \"custom\" : { \"number\" : 42 },\n  \"hooks\" : {\"SessionStart\":[{\"matcher\":\"startup\",\"hooks\":[{\"type\":\"command\",\"command\":\"/usr/bin/other\"}]}]}\n}";
    write_config(&f, original);
    install(&f);
    let installed = fs::read(config(&f)).expect("installed");
    assert!(std::str::from_utf8(&installed).expect("utf8").starts_with(
        "{\n  \"description\" : \"exact whitespace stays\",\n  \"custom\" : { \"number\" : 42 },"
    ));
    let document = parse_config(&installed).expect("valid");
    assert_eq!(
        document["hooks"]["SessionStart"]
            .as_array()
            .expect("groups")
            .len(),
        2
    );
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("again");
    let id = plan.preview().plan_id;
    assert_eq!(f.installer.apply(plan, id), Ok(InstallOutcome::Unchanged));
    assert_eq!(fs::read(config(&f)).expect("unchanged"), installed);
    let plan = f
        .installer
        .prepare_disconnect(&f.project)
        .expect("disconnect");
    let id = plan.preview().plan_id;
    assert_eq!(f.installer.apply(plan, id), Ok(InstallOutcome::Changed));
    assert_eq!(
        parse_config(&fs::read(config(&f)).expect("after")).expect("valid"),
        parse_config(original.as_bytes()).expect("original")
    );
}

#[test]
fn fresh_config_is_removed_only_when_it_contains_no_user_data() {
    let f = fixture();
    install(&f);
    let plan = f.installer.prepare_disconnect(&f.project).expect("plan");
    let id = plan.preview().plan_id;
    f.installer.apply(plan, id).expect("remove");
    assert!(!config(&f).exists());
    install(&f);
    let mut value = parse_config(&fs::read(config(&f)).expect("config")).expect("parse");
    value["description"] = json!("new user data");
    fs::write(config(&f), serde_json::to_vec(&value).expect("json")).expect("edit");
    let plan = f.installer.prepare_disconnect(&f.project).expect("plan");
    let id = plan.preview().plan_id;
    f.installer.apply(plan, id).expect("remove");
    assert_eq!(
        parse_config(&fs::read(config(&f)).expect("retained")).expect("parse")["description"],
        "new user data"
    );
}

#[test]
fn upgrade_and_rollback_remove_only_owned_entries_after_user_edits() {
    let mut f = fixture();
    install(&f);
    let original_command = f
        .installer
        .prepare_install(f.request.clone())
        .expect("preview")
        .preview()
        .owned_command
        .clone();
    f.request.policy_revision = 2;
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("upgrade");
    let id = plan.preview().plan_id;
    assert_eq!(plan.preview().action, InstallAction::Upgrade);
    f.installer.apply(plan, id).expect("upgrade");
    let mut value = parse_config(&fs::read(config(&f)).expect("config")).expect("parse");
    value["hooks"]["Stop"]
        .as_array_mut()
        .expect("groups")
        .push(json!({"hooks":[{"type":"command","command":"/usr/bin/user-new-hook"}]}));
    fs::write(config(&f), serde_json::to_vec(&value).expect("json")).expect("edit");
    let plan = f.installer.prepare_rollback(&f.project).expect("rollback");
    let id = plan.preview().plan_id;
    f.installer.apply(plan, id).expect("apply");
    let content = fs::read_to_string(config(&f)).expect("config");
    assert!(content.contains("/usr/bin/user-new-hook"));
    assert!(content.contains(&original_command.replace('\\', "\\\\")));
    assert!(!content.contains("--policy-revision 2"));
}

#[test]
fn concurrent_edit_and_changed_helper_leave_user_configuration_untouched() {
    let f = fixture();
    write_config(&f, "{}");
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("plan");
    let id = plan.preview().plan_id;
    write_config(&f, "{\"description\":\"user edit\"}");
    assert_eq!(
        f.installer.apply(plan, id),
        Err(InstallError::ConcurrentEdit)
    );
    let original = fs::read(config(&f)).expect("config");
    let plan = f
        .installer
        .prepare_install(f.request.clone())
        .expect("plan");
    let id = plan.preview().plan_id;
    fs::write(
        &f.request.helper_path,
        "#!/bin/sh\n# changed\necho 'mochi-hook 0.1.0'\n",
    )
    .expect("helper");
    assert_eq!(
        f.installer.apply(plan, id),
        Err(InstallError::ConcurrentEdit)
    );
    assert_eq!(fs::read(config(&f)).expect("after"), original);
}

#[test]
fn edited_owned_handler_is_a_conflict_instead_of_an_unsafe_uninstall() {
    let f = fixture();
    install(&f);
    let mut value = parse_config(&fs::read(config(&f)).expect("config")).expect("parse");
    value["hooks"]["Stop"][0]["hooks"][0]["timeout"] = json!(2);
    let bytes = serde_json::to_vec(&value).expect("json");
    fs::write(config(&f), &bytes).expect("edit");
    assert!(matches!(
        f.installer.prepare_disconnect(&f.project),
        Err(InstallError::OwnershipConflict)
    ));
    assert_eq!(fs::read(config(&f)).expect("after"), bytes);
}

#[test]
fn malformed_duplicate_oversized_and_inline_hook_configuration_fail_closed() {
    for text in ["{", "{\"hooks\":{},\"hooks\":{}}", "{\"hooks\":{\"Stop\":123}}", "{\"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"mochi-hook capture\"}]}]}}"] {
        let f = fixture(); write_config(&f, text);
        assert!(f.installer.prepare_install(f.request.clone()).is_err()); assert_eq!(fs::read_to_string(config(&f)).expect("config"), text);
    }
    let f = fixture();
    let text = format!(
        "{{\"description\":\"{}\"}}",
        "x".repeat(MAX_CONFIG_BYTES as usize)
    );
    write_config(&f, &text);
    assert!(matches!(
        f.installer.prepare_install(f.request.clone()),
        Err(InstallError::Unreadable)
    ));
    write_config(&f, "{}");
    let toml = f.project.join(".codex/config.toml");
    fs::write(&toml, "['hooks'.Stop]\nmatcher = '*'\n").expect("toml");
    assert!(matches!(
        f.installer.prepare_install(f.request.clone()),
        Err(InstallError::UnsupportedConfiguration)
    ));
    assert_eq!(
        fs::read_to_string(toml).expect("toml"),
        "['hooks'.Stop]\nmatcher = '*'\n"
    );
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected_and_private_artifacts_have_owner_permissions() {
    use std::os::unix::fs::symlink;
    let f = fixture();
    install(&f);
    assert_eq!(
        fs::metadata(config(&f)).expect("mode").permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&f.installer.state_root)
            .expect("mode")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for entry in fs::read_dir(&f.installer.state_root).expect("state") {
        let entry = entry.expect("entry");
        let meta = entry.metadata().expect("metadata");
        assert_eq!(
            meta.permissions().mode() & 0o777,
            if meta.is_dir() { 0o700 } else { 0o600 }
        );
    }
    let plan = f.installer.prepare_disconnect(&f.project).expect("plan");
    let id = plan.preview().plan_id;
    f.installer.apply(plan, id).expect("remove");
    let other = f.root.join("other.json");
    fs::write(&other, "{}").expect("other");
    symlink(&other, config(&f)).expect("symlink");
    assert!(matches!(
        f.installer.prepare_install(f.request.clone()),
        Err(InstallError::UnsafePath)
    ));
    assert_eq!(fs::read(other).expect("other"), b"{}");
}

#[test]
fn backup_cleanup_removes_expired_owned_names_and_does_not_read_content() {
    let f = fixture();
    let backups = f.installer.state_root.join("backups");
    let old = backups.join(format!("1-{}.json", Uuid::new_v4()));
    fs::write(&old, "synthetic configuration recovery only").expect("backup");
    let unrelated = backups.join("user-file.json");
    fs::write(&unrelated, "keep").expect("other");
    assert_eq!(
        f.installer
            .cleanup_backups(UNIX_EPOCH + Duration::from_secs(8 * 86400))
            .expect("cleanup"),
        1
    );
    assert!(!old.exists());
    assert!(unrelated.exists());
}

#[test]
fn pending_journal_recovers_before_and_after_configuration_commit() {
    for committed in [false, true] {
        let f = fixture();
        write_config(&f, "{}");
        let plan = f
            .installer
            .prepare_install(f.request.clone())
            .expect("plan");
        let mut receipt = plan.receipt.clone();
        receipt.pending = Some(Pending {
            before_hash: digest(&plan.before),
            after_hash: digest(&plan.after),
            next: plan.next.clone(),
            previous: None,
        });
        f.installer.save_receipt(&receipt).expect("journal");
        if committed {
            write_atomic(&config(&f), plan.after.as_ref().expect("after")).expect("write");
        }
        let next = f
            .installer
            .prepare_install(f.request.clone())
            .expect("recover");
        assert_eq!(next.preview().changes_configuration, !committed);
        assert!(next.receipt.pending.is_none());
    }
}

#[test]
fn two_prepared_installations_cannot_overwrite_each_other() {
    let f = fixture();
    let a = f.installer.prepare_install(f.request.clone()).expect("a");
    let a_id = a.preview().plan_id;
    let b = f.installer.prepare_install(f.request.clone()).expect("b");
    let b_id = b.preview().plan_id;
    f.installer.apply(a, a_id).expect("a committed");
    assert_eq!(
        f.installer.apply(b, b_id),
        Err(InstallError::ConcurrentEdit)
    );
}
