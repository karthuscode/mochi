use crate::DetectionError;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MAX_OUTPUT_BYTES: usize = 16 * 1024;

pub(crate) fn run_bounded(
    executable: &Path,
    arguments: &[&str],
    timeout: Duration,
) -> Result<String, DetectionError> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(map_spawn_error)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(DetectionError::ExecutableFailed)?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take((MAX_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(DetectionError::ExecutableTimedOut);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(DetectionError::ExecutableFailed);
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| DetectionError::ExecutableFailed)?
        .map_err(|_| DetectionError::ExecutableFailed)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(DetectionError::OutputLimitExceeded);
    }
    if !status.success() {
        return Err(DetectionError::ExecutableFailed);
    }
    String::from_utf8(bytes).map_err(|_| DetectionError::VersionParseFailed)
}

fn map_spawn_error(error: std::io::Error) -> DetectionError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        DetectionError::PermissionDenied
    } else {
        DetectionError::ExecutableFailed
    }
}
