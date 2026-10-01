use crate::DetectionError;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
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
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(map_spawn_error)?;
    let stdout = child
        .stdout
        .take()
        .ok_or(DetectionError::ExecutableFailed)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take((MAX_OUTPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let deadline = Instant::now() + timeout;
    let mut output = None;
    let result = loop {
        if output.is_none() {
            match receiver.try_recv() {
                Ok(Ok(bytes)) if bytes.len() > MAX_OUTPUT_BYTES => {
                    break Err(DetectionError::OutputLimitExceeded);
                }
                Ok(Ok(bytes)) => output = Some(bytes),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    break Err(DetectionError::ExecutableFailed);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if !status.success() => break Err(DetectionError::ExecutableFailed),
            Ok(Some(_)) if output.is_some() => {
                break String::from_utf8(output.take().unwrap_or_default())
                    .map_err(|_| DetectionError::VersionParseFailed);
            }
            Err(_) => break Err(DetectionError::ExecutableFailed),
            _ => {}
        }
        if Instant::now() >= deadline {
            break Err(DetectionError::ExecutableTimedOut);
        }
        thread::sleep(Duration::from_millis(5));
    };
    // A descendant can inherit stdout after the direct child exits. Never join a
    // pipe reader without a deadline; terminate our isolated process group.
    #[cfg(unix)]
    if let Ok(group) = i32::try_from(child.id()) {
        // SAFETY: group is the positive PID of our own process_group(0) child.
        unsafe { libc::kill(-group, libc::SIGKILL) };
    }
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn map_spawn_error(error: std::io::Error) -> DetectionError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        DetectionError::PermissionDenied
    } else {
        DetectionError::ExecutableFailed
    }
}
