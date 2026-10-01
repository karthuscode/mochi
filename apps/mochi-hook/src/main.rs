use mochi_capture::{
    CaptureSanitizer, ClientSurface, CodexSessionSource, GitCliContextReader, GitContextReader,
    SessionSource, SnapshotRole, Spool, SpoolLimits, SystemClock,
};
use mochi_integration::{CodexInstaller, InstallRequest};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{BufRead, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_STDIN_BYTES: u64 = 1024 * 1024 + 1;

fn main() {
    let mode = std::env::args().nth(1);
    if matches!(mode.as_deref(), Some("capture" | "capture-authorized")) {
        std::panic::set_hook(Box::new(|_| {}));
        let _ = catch_unwind(AssertUnwindSafe(|| {
            if mode.as_deref() == Some("capture-authorized") {
                run_authorized_capture();
            } else {
                run_capture();
            }
        }));
        return;
    }
    let code = match mode.as_deref() {
        Some("version") => {
            println!("mochi-hook {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("inspect") => run_inspect(),
        Some("git-snapshot") => run_git_snapshot(),
        Some("connect") => run_integration("connect"),
        Some("disconnect") => run_integration("disconnect"),
        Some("rollback-integration") => run_integration("rollback"),
        _ => Err("usage"),
    };
    if let Err(message) = code {
        eprintln!("mochi-hook: {message}");
        std::process::exit(2);
    }
}

/// Explicit developer connection flow, separate from silent capture. Existing
/// config and backups never appear in the preview or error output.
fn run_integration(action: &str) -> Result<(), &'static str> {
    let options = parse_options(std::env::args().skip(2))?;
    let root = option(&options, "approved-root")
        .map(PathBuf::from)
        .ok_or("approved root required")?;
    let state_root = option(&options, "integration-state-root")
        .map(PathBuf::from)
        .ok_or("private integration state required")?;
    let installer = CodexInstaller::new(state_root).map_err(|_| "integration state unavailable")?;
    let plan = match action {
        "connect" => {
            let project_id = option(&options, "project-id")
                .and_then(|v| Uuid::parse_str(v).ok())
                .ok_or("project id required")?;
            let policy_revision = option(&options, "policy-revision")
                .and_then(|v| v.parse::<u64>().ok())
                .ok_or("policy revision required")?;
            let spool_root = option(&options, "spool-root")
                .map(PathBuf::from)
                .ok_or("private spool root required")?;
            let helper_path = std::env::current_exe().map_err(|_| "helper unavailable")?;
            installer.prepare_install(InstallRequest {
                project_id,
                approved_root: root,
                helper_path,
                spool_root,
                policy_revision,
            })
        }
        "disconnect" => installer.prepare_disconnect(&root),
        _ => installer.prepare_rollback(&root),
    }
    .map_err(|error| match error {
        mochi_integration::InstallError::ConcurrentEdit => "configuration changed; preview again",
        mochi_integration::InstallError::OwnershipConflict => {
            "owned hook conflict; manual review required"
        }
        mochi_integration::InstallError::UnsupportedConfiguration => {
            "inline hooks require manual review"
        }
        mochi_integration::InstallError::RecoveryRequired => {
            "integration recovery requires manual review"
        }
        mochi_integration::InstallError::NotInstalled => "no owned installation",
        _ => "integration preview unavailable",
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(plan.preview()).map_err(|_| "preview unavailable")?
    );
    println!("This changes only the displayed project hooks. Connecting permits local capture within this approved root. Remote analysis remains off; no API key or account settings are changed.");
    println!("Configuration backups are private recovery files and expire at the next cleanup after seven days. Codex hook trust must be reviewed through /hooks; it is never changed here.");
    let approval = format!("approve {}", plan.preview().plan_id);
    println!("To approve this exact change, type: {approval}");
    let mut input = String::new();
    std::io::stdin()
        .lock()
        .take(128)
        .read_line(&mut input)
        .map_err(|_| "approval unavailable")?;
    if input.len() >= 128 || input.trim() != approval {
        println!("No configuration change approved.");
        return Ok(());
    }
    let id = plan.preview().plan_id;
    let outcome = installer
        .apply(plan, id)
        .map_err(|_| "integration change failed safely; preview again or review recovery")?;
    println!("Integration configuration: {outcome:?}. Review the exact Mochi hooks in Codex /hooks before testing.");
    Ok(())
}

fn run_capture() {
    let Ok(options) = parse_options(std::env::args().skip(2)) else {
        return;
    };
    let Some(project_id) =
        option(&options, "project-id").and_then(|value| Uuid::parse_str(value).ok())
    else {
        return;
    };
    let Some(approved_root) = option(&options, "approved-root")
        .map(PathBuf::from)
        .and_then(|path| path.canonicalize().ok())
    else {
        return;
    };
    let Some(surface) = option(&options, "client-surface").and_then(ClientSurface::parse) else {
        return;
    };
    let Some(policy_revision) =
        option(&options, "policy-revision").and_then(|value| value.parse::<u64>().ok())
    else {
        return;
    };
    let spool_root = match option(&options, "spool-root") {
        Some(value) => PathBuf::from(value),
        None => match Spool::default_root() {
            Ok(value) => value,
            Err(_) => return,
        },
    };
    capture_normalized(
        project_id,
        approved_root,
        surface,
        policy_revision,
        spool_root,
    );
}

fn run_authorized_capture() {
    use mochi_capture::authorization::AuthorizationStore;
    let Ok(options) = parse_options(std::env::args().skip(2)) else {
        return;
    };
    if options.len() != 2 {
        return;
    }
    let Some(id) = option(&options, "project-id").and_then(|v| Uuid::parse_str(v).ok()) else {
        return;
    };
    let Some(root) = option(&options, "policy-root").map(PathBuf::from) else {
        return;
    };
    let Ok(store) = AuthorizationStore::existing(root) else {
        return;
    };
    let Ok(lease) = store.authorize(id) else {
        return;
    };
    let policy = lease.policy();
    capture_normalized(
        id,
        policy.approved_root.clone(),
        ClientSurface::Cli,
        policy.policy_revision,
        policy.spool_root.clone(),
    );
    drop(lease);
}

fn capture_normalized(
    project_id: Uuid,
    approved_root: PathBuf,
    surface: ClientSurface,
    policy_revision: u64,
    spool_root: PathBuf,
) {
    let Ok(sanitizer) = CaptureSanitizer::new(&approved_root) else {
        return;
    };
    let source = CodexSessionSource::new(
        project_id,
        approved_root,
        surface,
        policy_revision,
        sanitizer,
        SystemClock,
    );
    let Some(raw) = bounded_capture_stdin() else {
        return;
    };
    let pending = match source.normalize(&raw) {
        Ok(events) => events,
        Err(error) => vec![source.metadata_gap(&error)],
    };
    let received_at = source.received_at();
    let spool = Spool::new(spool_root, SpoolLimits::default());
    let _ = spool.append(
        source.project_id(),
        source.descriptor(),
        &received_at,
        pending,
    );
}

/// Poll the inherited pipe with a total deadline; an open writer must never
/// keep a Codex hook waiting. Timed-out input is discarded without durable text.
#[cfg(unix)]
fn bounded_capture_stdin() -> Option<Vec<u8>> {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_millis(750);
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline.checked_duration_since(Instant::now())?;
        let mut descriptor = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        // Only this thread reads stdin; poll gates each bounded read.
        let ready = unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis().max(1) as i32) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        if ready <= 0 || descriptor.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
            return None;
        }
        let count =
            unsafe { libc::read(libc::STDIN_FILENO, chunk.as_mut_ptr().cast(), chunk.len()) };
        if count < 0 {
            return None;
        }
        if count == 0 {
            return Some(bytes);
        }
        bytes.extend_from_slice(&chunk[..count as usize]);
        if bytes.len() as u64 >= MAX_STDIN_BYTES {
            return Some(bytes);
        }
    }
}
#[cfg(not(unix))]
fn bounded_capture_stdin() -> Option<Vec<u8>> {
    None
}

fn run_inspect() -> Result<(), &'static str> {
    let options = parse_options(std::env::args().skip(2))?;
    let root = match option(&options, "spool-root") {
        Some(value) => PathBuf::from(value),
        None => Spool::default_root().map_err(|_| "spool path unavailable")?,
    };
    let spool = Spool::new(root, SpoolLimits::default());
    let events = spool
        .read_records()
        .map_err(|_| "spool is unavailable or corrupt")?;
    let state = spool.read_state().map_err(|_| "spool state is corrupt")?;
    let output = json!({
        "schemaVersion": 1,
        "spoolState": state,
        "events": events,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).map_err(|_| "serialization failed")?
    );
    Ok(())
}

fn run_git_snapshot() -> Result<(), &'static str> {
    let options = parse_options(std::env::args().skip(2))?;
    let project_id = option(&options, "project-id")
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or("invalid project id")?;
    let approved_root = option(&options, "approved-root")
        .map(PathBuf::from)
        .ok_or("approved root is required")?;
    let role = option(&options, "role")
        .and_then(SnapshotRole::parse)
        .ok_or("invalid snapshot role")?;
    let output = option(&options, "output")
        .map(PathBuf::from)
        .ok_or("output path is required")?;
    let snapshot = GitCliContextReader
        .snapshot(project_id, &approved_root, role)
        .map_err(|_| "Git snapshot failed")?;
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(|_| "serialization failed")?;
    write_private_atomic(&output, &bytes).map_err(|_| "snapshot write failed")
}

fn parse_options(
    args: impl Iterator<Item = String>,
) -> Result<BTreeMap<String, String>, &'static str> {
    let values = args.collect::<Vec<_>>();
    let (pairs, remainder) = values.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err("options must be --name value pairs");
    }
    let mut options = BTreeMap::new();
    for pair in pairs {
        let name = pair[0].strip_prefix("--").ok_or("invalid option")?;
        if !matches!(
            name,
            "project-id"
                | "approved-root"
                | "client-surface"
                | "policy-revision"
                | "spool-root"
                | "role"
                | "output"
                | "integration-state-root"
                | "policy-root"
        ) {
            return Err("unknown option");
        }
        if options.insert(name.to_owned(), pair[1].clone()).is_some() {
            return Err("duplicate option");
        }
    }
    Ok(options)
}

fn option<'a>(options: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    options.get(name).map(String::as_str)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".mochi-snapshot-{}", Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_data()?;
    std::fs::rename(temporary, path)
}
