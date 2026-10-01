use mochi_capture::{
    CaptureSanitizer, ClientSurface, CodexSessionSource, GitCliContextReader, GitContextReader,
    SessionSource, SnapshotRole, Spool, SpoolLimits, SystemClock,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_STDIN_BYTES: u64 = 1024 * 1024 + 1;

fn main() {
    let mode = std::env::args().nth(1);
    if mode.as_deref() == Some("capture") {
        std::panic::set_hook(Box::new(|_| {}));
        let _ = catch_unwind(AssertUnwindSafe(run_capture));
        return;
    }
    let code = match mode.as_deref() {
        Some("version") => {
            println!("mochi-hook {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("inspect") => run_inspect(),
        Some("git-snapshot") => run_git_snapshot(),
        _ => Err("usage"),
    };
    if let Err(message) = code {
        eprintln!("mochi-hook: {message}");
        std::process::exit(2);
    }
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
    let mut raw = Vec::new();
    if std::io::stdin()
        .take(MAX_STDIN_BYTES)
        .read_to_end(&mut raw)
        .is_err()
    {
        return;
    }
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
