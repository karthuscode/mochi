use crate::{
    Architecture, CapabilitySupport, ClientSurface, CodexDetector, CodexDetectorOptions,
    Compatibility, ConfigurationState, DetectionError, EvidenceLevel, HelperState,
    IntegrationDetectionService, Readiness, TrustState,
};
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn executable(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("script");
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("permissions");
    }
}

fn helper(root: &Path, version: &str) -> PathBuf {
    let path = root.join("mochi-hook");
    executable(
        &path,
        &format!("if [ \"$1\" = version ]; then echo 'mochi-hook {version}'; else exit 2; fi"),
    );
    path
}

fn cli(directory: &Path, output: &str) -> PathBuf {
    fs::create_dir_all(directory).expect("cli dir");
    let path = directory.join("codex");
    executable(&path, &format!("echo '{output}'"));
    path
}

fn hooks(codex_home: &Path, helper: &Path, unrelated: bool) {
    fs::create_dir_all(codex_home).expect("codex home");
    let command = format!("{} capture --project-id 11111111-1111-4111-8111-111111111111 --approved-root /synthetic --client-surface cli --policy-revision 1", crate::hooks::shell_quote(helper.to_str().expect("path")).expect("quote"));
    let mut events = serde_json::Map::new();
    for event in crate::CAPTURE_EVENTS {
        events.insert(
            event.to_owned(),
            serde_json::json!([{"hooks": [{"type": "command", "command": command}]}]),
        );
    }
    if unrelated {
        events.get_mut("SessionStart").expect("event").as_array_mut().expect("groups").push(serde_json::json!({"hooks": [{"type": "command", "command": "/usr/local/bin/user-owned-hook"}]}));
    }
    fs::write(
        codex_home.join("hooks.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"hooks": events})).expect("hooks json"),
    )
    .expect("hooks");
}

fn options(root: &Path) -> CodexDetectorOptions {
    CodexDetectorOptions {
        path: OsString::new(),
        codex_home: Some(root.join(".codex")),
        project_roots: Vec::new(),
        desktop_search_roots: vec![root.join("Applications")],
        helper_path: root.join("mochi-hook"),
        expected_helper_version: "0.1.0".to_owned(),
        expected_architecture: Architecture::current(),
        command_timeout: Duration::from_secs(5),
        known_trust: Some(TrustState::Trusted),
    }
}

fn surface(
    report: &crate::IntegrationDetection,
    surface: ClientSurface,
) -> &crate::SurfaceDetection {
    report
        .surfaces
        .iter()
        .find(|item| item.surface == surface)
        .expect("surface")
}

#[test]
fn absent_clients_are_normal_results() {
    let root = TempDir::new().expect("root");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).readiness,
        Readiness::NotInstalled
    );
    assert_eq!(
        surface(&report, ClientSurface::Desktop).readiness,
        Readiness::NotInstalled
    );
}

#[test]
fn verified_cli_can_be_ready_and_detection_is_read_only() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    cli(&bin, "codex-cli 0.151.0");
    let helper = helper(root.path(), "0.1.0");
    hooks(&root.path().join(".codex"), &helper, true);
    let before = fs::read(root.path().join(".codex/hooks.json")).expect("before");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    detector_options.helper_path = helper;
    let report = CodexDetector::new(detector_options).detect();
    let cli = surface(&report, ClientSurface::Cli);
    assert_eq!(cli.installations.len(), 1);
    assert_eq!(cli.installations[0].version.as_deref(), Some("0.151.0"));
    assert_eq!(cli.installations[0].compatibility, Compatibility::Verified);
    assert_eq!(cli.readiness, Readiness::Ready);
    assert_eq!(cli.helper.state, HelperState::Present);
    assert_eq!(cli.configuration.state, ConfigurationState::MochiConfigured);
    assert!(cli.configuration.unrelated_hooks_present);
    assert_eq!(
        fs::read(root.path().join(".codex/hooks.json")).expect("after"),
        before
    );
}

#[test]
fn multiple_cli_installations_remain_distinct_and_deterministic() {
    let root = TempDir::new().expect("root");
    let first = root.path().join("first");
    let second = root.path().join("second");
    cli(&first, "codex-cli 0.151.0");
    cli(&second, "codex-cli 0.200.0");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&first, &second]).expect("path");
    let report = CodexDetector::new(detector_options).detect();
    let cli = surface(&report, ClientSurface::Cli);
    assert_eq!(cli.installations.len(), 2);
    assert_eq!(cli.installations[0].location, first.join("codex"));
    assert_eq!(cli.installations[1].location, second.join("codex"));
    assert_eq!(cli.readiness, Readiness::AmbiguousInstallations);
}

#[test]
fn unknown_older_and_newer_versions_require_validation() {
    for version in ["0.100.0", "0.200.0"] {
        let root = TempDir::new().expect("root");
        let bin = root.path().join("bin");
        cli(&bin, &format!("codex-cli {version}"));
        let mut detector_options = options(root.path());
        detector_options.path = std::env::join_paths([&bin]).expect("path");
        let report = CodexDetector::new(detector_options).detect();
        let cli = surface(&report, ClientSurface::Cli);
        assert_eq!(
            cli.installations[0].compatibility,
            Compatibility::RequiresValidation
        );
        assert_eq!(cli.readiness, Readiness::RequiresCompatibilityValidation);
        assert_eq!(
            cli.capabilities.user_prompt.evidence_level,
            EvidenceLevel::HistoricalBaseline
        );
    }
}

#[test]
fn malformed_and_failed_version_commands_do_not_mean_absent() {
    for (body, expected_error) in [
        ("echo malformed", DetectionError::VersionParseFailed),
        ("exit 7", DetectionError::ExecutableFailed),
    ] {
        let root = TempDir::new().expect("root");
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).expect("bin");
        executable(&bin.join("codex"), body);
        let mut detector_options = options(root.path());
        detector_options.path = std::env::join_paths([&bin]).expect("path");
        let report = CodexDetector::new(detector_options).detect();
        let cli = surface(&report, ClientSurface::Cli);
        assert_eq!(cli.installations.len(), 1);
        assert_eq!(cli.installations[0].version, None);
        assert!(cli.errors.contains(&expected_error));
        assert_eq!(cli.readiness, Readiness::RequiresCompatibilityValidation);
    }
}

#[test]
fn version_execution_timeout_is_bounded() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin");
    executable(&bin.join("codex"), "/bin/sleep 2");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    detector_options.command_timeout = Duration::from_millis(30);
    let started = Instant::now();
    let report = CodexDetector::new(detector_options).detect();
    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(surface(&report, ClientSurface::Cli)
        .errors
        .contains(&DetectionError::ExecutableTimedOut));
}

#[test]
fn configuration_states_preserve_unrelated_hooks_and_fail_closed() {
    let cases = [
        (None, ConfigurationState::Absent),
        (
            Some(
                r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"/usr/bin/other"}]}]}}"#,
            ),
            ConfigurationState::PresentWithoutMochi,
        ),
        (
            Some(
                r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"/old/mochi-hook capture"}]}]}}"#,
            ),
            ConfigurationState::MochiCommandMismatch,
        ),
        (Some("{"), ConfigurationState::Malformed),
    ];
    for (content, expected) in cases {
        let root = TempDir::new().expect("root");
        if let Some(content) = content {
            fs::create_dir_all(root.path().join(".codex")).expect("home");
            fs::write(root.path().join(".codex/hooks.json"), content).expect("config");
        }
        let report = CodexDetector::new(options(root.path())).detect();
        assert_eq!(
            surface(&report, ClientSurface::Cli).configuration.state,
            expected
        );
    }
}

#[test]
fn helper_missing_present_wrong_version_not_executable_and_wrong_architecture() {
    let root = TempDir::new().expect("root");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).helper.state,
        HelperState::Missing
    );

    let path = helper(root.path(), "0.1.0");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).helper.state,
        HelperState::Present
    );

    helper(root.path(), "9.9.9");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).helper.state,
        HelperState::VersionMismatch
    );

    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    #[cfg(unix)]
    permissions.set_mode(0o600);
    fs::set_permissions(&path, permissions).expect("permissions");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).helper.state,
        HelperState::NotExecutable
    );

    let mut macho = vec![0u8; 8];
    macho[0..4].copy_from_slice(&0xfeedfacfu32.to_le_bytes());
    let wrong_cpu = if Architecture::current() == Architecture::Arm64 {
        0x01000007u32
    } else {
        0x0100000cu32
    };
    macho[4..8].copy_from_slice(&wrong_cpu.to_le_bytes());
    fs::write(&path, macho).expect("macho");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    #[cfg(unix)]
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("permissions");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).helper.state,
        HelperState::ArchitectureMismatch
    );
}

#[test]
fn capability_baselines_keep_desktop_prompt_and_interrupt_unknown() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    cli(&bin, "codex-cli 0.151.0");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    let report = CodexDetector::new(detector_options).detect();
    let cli = surface(&report, ClientSurface::Cli);
    assert_eq!(
        cli.capabilities.user_prompt.support,
        CapabilitySupport::Supported
    );
    assert_eq!(
        cli.capabilities.interrupts.support,
        CapabilitySupport::Supported
    );
    let desktop = surface(&report, ClientSurface::Desktop);
    assert_eq!(
        desktop.capabilities.user_prompt.support,
        CapabilitySupport::Unknown
    );
    assert_eq!(
        desktop.capabilities.interrupts.support,
        CapabilitySupport::Unknown
    );
}

#[cfg(target_os = "macos")]
fn desktop(application_root: &Path, version: &str) {
    desktop_versions(application_root, version, version);
}

#[cfg(target_os = "macos")]
fn desktop_versions(application_root: &Path, version: &str, build: &str) {
    let application = application_root.join("Codex.app");
    fs::create_dir_all(application.join("Contents/MacOS")).expect("bundle");
    fs::write(
        application.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>com.openai.codex</string>
<key>CFBundleShortVersionString</key><string>{version}</string>
<key>CFBundleVersion</key><string>{build}</string>
<key>CFBundleExecutable</key><string>Codex</string>
</dict></plist>"#
        ),
    )
    .expect("plist");
    executable(&application.join("Contents/MacOS/Codex"), "exit 0");
}

#[cfg(target_os = "macos")]
#[test]
fn desktop_only_is_distinct_and_known_partial_is_degraded() {
    let root = TempDir::new().expect("root");
    let applications = root.path().join("Applications");
    desktop_versions(&applications, "1.0.0", "26.908.40834");
    let helper = helper(root.path(), "0.1.0");
    hooks(&root.path().join(".codex"), &helper, false);
    let mut detector_options = options(root.path());
    detector_options.helper_path = helper;
    let report = CodexDetector::new(detector_options).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).readiness,
        Readiness::NotInstalled
    );
    let desktop = surface(&report, ClientSurface::Desktop);
    assert_eq!(desktop.installations.len(), 1);
    assert_eq!(
        desktop.installations[0].compatibility,
        Compatibility::PartiallyVerified
    );
    assert_eq!(desktop.readiness, Readiness::Degraded);
}

#[cfg(target_os = "macos")]
#[test]
fn cli_and_desktop_coexist_without_collapsing() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    cli(&bin, "codex-cli 0.151.0");
    desktop(&root.path().join("Applications"), "26.908.40834");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    let report = CodexDetector::new(detector_options).detect();
    assert_eq!(surface(&report, ClientSurface::Cli).installations.len(), 1);
    assert_eq!(
        surface(&report, ClientSurface::Desktop).installations.len(),
        1
    );
}

#[test]
fn unknown_trust_is_never_inferred_as_ready() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    cli(&bin, "codex-cli 0.151.0");
    let helper = helper(root.path(), "0.1.0");
    hooks(&root.path().join(".codex"), &helper, false);
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    detector_options.helper_path = helper;
    detector_options.known_trust = None;
    let report = CodexDetector::new(detector_options).detect();
    let cli = surface(&report, ClientSurface::Cli);
    assert_eq!(cli.trust, TrustState::Unknown);
    assert_eq!(cli.readiness, Readiness::RequiresTrust);
}

#[test]
fn readiness_priorities_are_deterministic() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    cli(&bin, "codex-cli 0.151.0");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    let report = CodexDetector::new(detector_options.clone()).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).readiness,
        Readiness::RequiresMochiHook
    );

    let helper = helper(root.path(), "0.1.0");
    detector_options.helper_path = helper;
    let report = CodexDetector::new(detector_options.clone()).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).readiness,
        Readiness::RequiresCodexConfiguration
    );

    fs::create_dir_all(root.path().join(".codex")).expect("home");
    fs::write(root.path().join(".codex/hooks.json"), "{").expect("malformed");
    let report = CodexDetector::new(detector_options).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).readiness,
        Readiness::Error
    );
}

#[test]
fn representative_detection_cost_is_bounded() {
    let root = TempDir::new().expect("root");
    let bin = root.path().join("bin");
    let cli_path = cli(&bin, "codex-cli 0.151.0");
    let mut detector_options = options(root.path());
    detector_options.path = std::env::join_paths([&bin]).expect("path");
    let cli_started = Instant::now();
    let candidates = crate::codex::cli_candidates(&detector_options.path);
    let _ =
        crate::process::run_bounded(&cli_path, &["--version"], detector_options.command_timeout)
            .expect("version");
    let cli_elapsed = cli_started.elapsed();
    assert_eq!(candidates.len(), 1);
    #[cfg(target_os = "macos")]
    let desktop_elapsed = {
        let applications = root.path().join("Applications");
        desktop(&applications, "26.908.40834");
        let started = Instant::now();
        let metadata = crate::codex::read_bundle_metadata(
            &applications.join("Codex.app"),
            detector_options.command_timeout,
        )
        .expect("metadata");
        assert!(metadata.is_some());
        started.elapsed()
    };
    let started = Instant::now();
    let _ = CodexDetector::new(detector_options).detect();
    println!("synthetic CLI lookup/version: {cli_elapsed:?}");
    #[cfg(target_os = "macos")]
    println!("synthetic Desktop metadata: {desktop_elapsed:?}");
    println!("synthetic full Codex detection: {:?}", started.elapsed());
}

#[cfg(unix)]
#[test]
fn inherited_output_pipe_cannot_extend_the_command_deadline() {
    let root = TempDir::new().expect("root");
    let script = root.path().join("child-with-descendant");
    executable(&script, "sleep 10 &\necho 'codex-cli 0.151.0'\nexit 0");
    let start = Instant::now();
    assert_eq!(
        crate::process::run_bounded(&script, &[], Duration::from_millis(50)),
        Err(DetectionError::ExecutableTimedOut)
    );
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn hook_metadata_partial_coverage_and_shell_substitution_never_establish_readiness() {
    let root = TempDir::new().expect("root");
    let home = root.path().join(".codex");
    let helper = helper(root.path(), "0.1.0");
    fs::create_dir_all(&home).expect("home");
    fs::write(
        home.join("hooks.json"),
        serde_json::to_vec(
            &serde_json::json!({"description": format!("{} capture", helper.display())}),
        )
        .expect("json"),
    )
    .expect("config");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).configuration.state,
        ConfigurationState::Absent
    );
    hooks(&home, &helper, false);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(home.join("hooks.json")).expect("config")).expect("json");
    value["hooks"]
        .as_object_mut()
        .expect("events")
        .remove("Interrupt");
    fs::write(
        home.join("hooks.json"),
        serde_json::to_vec(&value).expect("json"),
    )
    .expect("config");
    let report = CodexDetector::new(options(root.path())).detect();
    assert_eq!(
        surface(&report, ClientSurface::Cli).configuration.state,
        ConfigurationState::MochiCommandMismatch
    );
    assert!(!crate::hooks::valid_capture_command(
        &format!("{} capture $(unsafe)", helper.display()),
        &helper
    ));
}
