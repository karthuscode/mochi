use crate::model::{
    CaptureGapPayload, CaptureGapReason, EventOrigin, NormalizedEvent, PendingEvent, Sensitivity,
    SensitivityClassification, SourceDescriptor, SpoolIngressRecord, INGRESS_SCHEMA_VERSION,
    SANITIZER_RULES_VERSION,
};
use fs2::FileExt;
use mochi_privacy::RedactionEngine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

pub const MAX_NORMALIZED_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_SPOOL_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_SPOOL_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const LOCK_ATTEMPTS: usize = 500;
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(1);
const MAX_SCAN_BATCH: usize = 100;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpoolAckToken {
    root: PathBuf,
    file_name: String,
}

impl SpoolAckToken {
    /// A non-reversible identifier suitable for metadata-only diagnostics.
    pub fn opaque_id(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"mochi-spool-token-v1\0");
        digest.update(self.root.to_string_lossy().as_bytes());
        digest.update(b"\0");
        digest.update(self.file_name.as_bytes());
        format!("{:x}", digest.finalize())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpoolRejectionReason {
    InvalidFilename,
    Oversized,
    Malformed,
    UnsupportedSchema,
    IdentityMismatch,
    NotRegularFile,
}

#[derive(Clone, Debug)]
pub enum SpoolCandidateData {
    Record(Box<SpoolIngressRecord>),
    Rejected {
        reason: SpoolRejectionReason,
        receive_sequence: Option<u64>,
    },
}

#[derive(Clone, Debug)]
pub struct SpoolCandidate {
    token: SpoolAckToken,
    data: SpoolCandidateData,
}

impl SpoolCandidate {
    pub fn token(&self) -> &SpoolAckToken {
        &self.token
    }

    pub fn data(&self) -> &SpoolCandidateData {
        &self.data
    }

    pub fn into_parts(self) -> (SpoolAckToken, SpoolCandidateData) {
        (self.token, self.data)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpoolLimits {
    pub max_event_bytes: usize,
    pub max_spool_bytes: u64,
    pub max_age: Duration,
}

impl Default for SpoolLimits {
    fn default() -> Self {
        Self {
            max_event_bytes: MAX_NORMALIZED_EVENT_BYTES,
            max_spool_bytes: MAX_SPOOL_BYTES,
            max_age: MAX_SPOOL_AGE,
        }
    }
}

#[derive(Debug)]
pub enum SpoolError {
    Io,
    Busy,
    InvalidState,
    Serialization,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpoolState {
    pub schema_version: u8,
    pub evicted_count: u64,
    pub last_eviction_at: Option<String>,
}

pub struct Spool {
    root: PathBuf,
    limits: SpoolLimits,
}

impl Spool {
    pub fn new(root: PathBuf, limits: SpoolLimits) -> Self {
        Self { root, limits }
    }

    pub fn default_root() -> Result<PathBuf, SpoolError> {
        let home = std::env::var_os("HOME").ok_or(SpoolError::InvalidState)?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("dev.mochi.desktop")
            .join("capture")
            .join("v1")
            .join("spool"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn append(
        &self,
        project_id: Uuid,
        source: SourceDescriptor,
        received_at: &str,
        pending: Vec<PendingEvent>,
    ) -> Result<Vec<SpoolIngressRecord>, SpoolError> {
        self.ensure_private_directory()?;
        let mut lock = self.open_private("stream.lock")?;
        let mut acquired = false;
        for _ in 0..LOCK_ATTEMPTS {
            match lock.try_lock_exclusive() {
                Ok(()) => {
                    acquired = true;
                    break;
                }
                Err(_) => thread::sleep(LOCK_RETRY_DELAY),
            }
        }
        if !acquired {
            return Err(SpoolError::Busy);
        }

        let reservation = (|| {
            let current = read_counter(&mut lock)?;
            let count = u64::try_from(pending.len()).map_err(|_| SpoolError::InvalidState)?;
            let reserved = current.checked_add(count).ok_or(SpoolError::InvalidState)?;
            write_counter(&mut lock, reserved)?;
            Ok(current)
        })();
        let _ = FileExt::unlock(&lock);
        let current = reservation?;

        let mut records = Vec::with_capacity(pending.len());
        for (offset, event) in pending.into_iter().enumerate() {
            let offset = u64::try_from(offset).map_err(|_| SpoolError::InvalidState)?;
            let sequence = current
                .checked_add(offset)
                .and_then(|value| value.checked_add(1))
                .ok_or(SpoolError::InvalidState)?;
            let record =
                self.make_record(project_id, source.clone(), received_at, sequence, event)?;
            self.write_record(&record)?;
            records.push(record);
        }

        // Pruning is serialized when the lock is immediately available. A busy writer may skip
        // this pass; the next successful invocation applies both size and age limits.
        if lock.try_lock_exclusive().is_ok() {
            let prune_result = self.prune(received_at);
            let _ = FileExt::unlock(&lock);
            prune_result?;
        }
        Ok(records)
    }

    pub fn read_records(&self) -> Result<Vec<SpoolIngressRecord>, SpoolError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut paths = event_paths(&self.root)?;
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let bytes = fs::read(path).map_err(|_| SpoolError::Io)?;
                if bytes.len() > self.limits.max_event_bytes {
                    return Err(SpoolError::InvalidState);
                }
                serde_json::from_slice(&bytes).map_err(|_| SpoolError::Serialization)
            })
            .collect()
    }

    /// Returns at most 100 entries in stable spool order. Invalid entries are represented by
    /// metadata-only rejections so a durable consumer can record the failure before acknowledging
    /// the exact file.
    pub fn scan_batch(&self, limit: usize) -> Result<Vec<SpoolCandidate>, SpoolError> {
        if limit == 0 || !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&self.root)
            .map_err(|_| SpoolError::Io)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".json") && name != "spool-state.json")
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        entries.truncate(limit.min(MAX_SCAN_BATCH));

        entries
            .into_iter()
            .map(|entry| {
                let file_name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| SpoolError::InvalidState)?;
                let token = SpoolAckToken {
                    root: self.root.clone(),
                    file_name: file_name.clone(),
                };
                let parsed_name = parse_event_filename(&file_name);
                let metadata = entry.metadata().map_err(|_| SpoolError::Io)?;
                let data = if !metadata.is_file() {
                    SpoolCandidateData::Rejected {
                        reason: SpoolRejectionReason::NotRegularFile,
                        receive_sequence: parsed_name.map(|value| value.0),
                    }
                } else if parsed_name.is_none() {
                    SpoolCandidateData::Rejected {
                        reason: SpoolRejectionReason::InvalidFilename,
                        receive_sequence: None,
                    }
                } else if metadata.len()
                    > u64::try_from(self.limits.max_event_bytes)
                        .map_err(|_| SpoolError::InvalidState)?
                {
                    SpoolCandidateData::Rejected {
                        reason: SpoolRejectionReason::Oversized,
                        receive_sequence: parsed_name.map(|value| value.0),
                    }
                } else {
                    let bytes = fs::read(entry.path()).map_err(|_| SpoolError::Io)?;
                    match serde_json::from_slice::<SpoolIngressRecord>(&bytes) {
                        Ok(record) if record.schema_version != INGRESS_SCHEMA_VERSION => {
                            SpoolCandidateData::Rejected {
                                reason: SpoolRejectionReason::UnsupportedSchema,
                                receive_sequence: parsed_name.map(|value| value.0),
                            }
                        }
                        Ok(record)
                            if parsed_name.is_some_and(|value| {
                                value != (record.receive_sequence, record.id)
                            }) =>
                        {
                            SpoolCandidateData::Rejected {
                                reason: SpoolRejectionReason::IdentityMismatch,
                                receive_sequence: parsed_name.map(|value| value.0),
                            }
                        }
                        Ok(record) => SpoolCandidateData::Record(Box::new(record)),
                        Err(_) => SpoolCandidateData::Rejected {
                            reason: SpoolRejectionReason::Malformed,
                            receive_sequence: parsed_name.map(|value| value.0),
                        },
                    }
                };
                Ok(SpoolCandidate { token, data })
            })
            .collect()
    }

    /// Removes only direct event-file tokens produced by this spool instance.
    pub fn acknowledge(&self, tokens: &[SpoolAckToken]) -> Result<(), SpoolError> {
        for token in tokens {
            if token.root != self.root || !is_direct_event_name(&token.file_name) {
                return Err(SpoolError::InvalidState);
            }
            let path = self.root.join(&token.file_name);
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err(SpoolError::Io),
            }
        }
        Ok(())
    }

    pub fn read_state(&self) -> Result<SpoolState, SpoolError> {
        let path = self.root.join("spool-state.json");
        if !path.exists() {
            return Ok(SpoolState {
                schema_version: 1,
                ..SpoolState::default()
            });
        }
        let bytes = fs::read(path).map_err(|_| SpoolError::Io)?;
        serde_json::from_slice(&bytes).map_err(|_| SpoolError::Serialization)
    }

    fn make_record(
        &self,
        project_id: Uuid,
        source: SourceDescriptor,
        received_at: &str,
        sequence: u64,
        event: PendingEvent,
    ) -> Result<SpoolIngressRecord, SpoolError> {
        let mut record = SpoolIngressRecord {
            schema_version: INGRESS_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            project_id,
            source,
            source_event_id: event.source_event_id,
            source_sequence: None,
            source_timestamp: None,
            received_at: received_at.to_owned(),
            receive_sequence: sequence,
            source_event_type: event.source_event_type,
            external_session_id: event.external_session_id,
            external_turn_id: event.external_turn_id,
            external_tool_use_id: event.external_tool_use_id,
            origin: EventOrigin::Provider,
            event: event.event,
            sensitivity: event.sensitivity,
        };
        let encoded = serde_json::to_vec(&record).map_err(|_| SpoolError::Serialization)?;
        if encoded.len() > self.limits.max_event_bytes {
            record.source_event_id = None;
            record.external_turn_id = None;
            record.external_tool_use_id = None;
            record.event = NormalizedEvent::CaptureGap(CaptureGapPayload {
                reason: CaptureGapReason::Overflow,
                from: None,
                to: None,
                dropped_count: Some(1),
            });
            record.sensitivity = Sensitivity {
                classification: SensitivityClassification::MetadataOnly,
                redaction_count: record.sensitivity.redaction_count,
                rules_version: SANITIZER_RULES_VERSION.to_owned(),
                policy_revision: record.sensitivity.policy_revision,
                truncated: true,
            };
            let fallback = serde_json::to_vec(&record).map_err(|_| SpoolError::Serialization)?;
            if fallback.len() > self.limits.max_event_bytes {
                return Err(SpoolError::InvalidState);
            }
        }
        let safe = RedactionEngine::new()
            .ok()
            .and_then(|engine| {
                serde_json::to_value(&record)
                    .ok()
                    .and_then(|value| engine.redact_value(&value).ok())
            })
            .is_some_and(|result| !result.changed);
        if !safe {
            record.source = SourceDescriptor {
                provider: "unknown".to_owned(),
                adapter_version: "unknown".to_owned(),
                transport: "unknown".to_owned(),
                client_surface: crate::model::ClientSurface::Unknown,
            };
            record.source_event_id = None;
            record.source_timestamp = None;
            record.source_event_type = "sanitization_failed".to_owned();
            record.external_session_id = None;
            record.external_turn_id = None;
            record.external_tool_use_id = None;
            record.received_at = "1970-01-01T00:00:00Z".to_owned();
            record.event = NormalizedEvent::CaptureGap(CaptureGapPayload {
                reason: CaptureGapReason::SanitizationFailed,
                from: None,
                to: None,
                dropped_count: Some(1),
            });
            record.sensitivity = Sensitivity {
                classification: SensitivityClassification::MetadataOnly,
                redaction_count: 0,
                rules_version: SANITIZER_RULES_VERSION.to_owned(),
                policy_revision: record.sensitivity.policy_revision,
                truncated: false,
            };
        }
        Ok(record)
    }

    fn write_record(&self, record: &SpoolIngressRecord) -> Result<(), SpoolError> {
        let bytes = serde_json::to_vec(record).map_err(|_| SpoolError::Serialization)?;
        if bytes.len() > self.limits.max_event_bytes {
            return Err(SpoolError::InvalidState);
        }
        let filename = format!("{:020}-{}.json", record.receive_sequence, record.id);
        let destination = self.root.join(filename);
        let temporary = self.root.join(format!(".tmp-{}", record.id));
        write_private_atomic(&temporary, &destination, &bytes)
    }

    fn prune(&self, received_at: &str) -> Result<(), SpoolError> {
        let mut entries = event_paths(&self.root)?
            .into_iter()
            .filter_map(|path| {
                let metadata = fs::metadata(&path).ok()?;
                Some((path, metadata.len(), metadata.modified().ok()))
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        let now = SystemTime::now();
        let mut total = entries.iter().map(|entry| entry.1).sum::<u64>();
        let mut evicted = 0u64;
        for (path, size, modified) in entries {
            let expired = modified
                .and_then(|value| now.duration_since(value).ok())
                .is_some_and(|age| age > self.limits.max_age);
            if (expired || total > self.limits.max_spool_bytes) && fs::remove_file(path).is_ok() {
                total = total.saturating_sub(size);
                evicted = evicted.saturating_add(1);
            }
        }
        if evicted > 0 {
            let mut state = self.read_state().unwrap_or(SpoolState {
                schema_version: 1,
                ..SpoolState::default()
            });
            state.schema_version = 1;
            state.evicted_count = state.evicted_count.saturating_add(evicted);
            state.last_eviction_at = Some(received_at.to_owned());
            let bytes = serde_json::to_vec(&state).map_err(|_| SpoolError::Serialization)?;
            let temporary = self.root.join(format!(".state-{}", Uuid::new_v4()));
            let destination = self.root.join("spool-state.json");
            write_private_atomic(&temporary, &destination, &bytes)?;
        }
        Ok(())
    }

    fn ensure_private_directory(&self) -> Result<(), SpoolError> {
        fs::create_dir_all(&self.root).map_err(|_| SpoolError::Io)?;
        #[cfg(unix)]
        fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
            .map_err(|_| SpoolError::Io)?;
        Ok(())
    }

    fn open_private(&self, name: &str) -> Result<File, SpoolError> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        options
            .open(self.root.join(name))
            .map_err(|_| SpoolError::Io)
    }
}

fn read_counter(file: &mut File) -> Result<u64, SpoolError> {
    file.seek(SeekFrom::Start(0)).map_err(|_| SpoolError::Io)?;
    let mut value = String::new();
    file.read_to_string(&mut value)
        .map_err(|_| SpoolError::Io)?;
    if value.trim().is_empty() {
        Ok(0)
    } else {
        value
            .trim()
            .parse::<u64>()
            .map_err(|_| SpoolError::InvalidState)
    }
}

fn write_counter(file: &mut File, value: u64) -> Result<(), SpoolError> {
    file.seek(SeekFrom::Start(0)).map_err(|_| SpoolError::Io)?;
    file.set_len(0).map_err(|_| SpoolError::Io)?;
    writeln!(file, "{value}").map_err(|_| SpoolError::Io)?;
    file.sync_data().map_err(|_| SpoolError::Io)
}

fn event_paths(root: &Path) -> Result<Vec<PathBuf>, SpoolError> {
    let entries = fs::read_dir(root).map_err(|_| SpoolError::Io)?;
    Ok(entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(".json") && name != "spool-state.json")
        })
        .collect())
}

fn parse_event_filename(name: &str) -> Option<(u64, Uuid)> {
    let stem = name.strip_suffix(".json")?;
    let (sequence, id) = stem.split_once('-')?;
    if sequence.len() != 20 || !sequence.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((sequence.parse().ok()?, Uuid::parse_str(id).ok()?))
}

fn is_direct_event_name(name: &str) -> bool {
    name != "spool-state.json"
        && name.ends_with(".json")
        && Path::new(name).file_name().and_then(|value| value.to_str()) == Some(name)
}

fn write_private_atomic(
    temporary: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), SpoolError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(temporary).map_err(|_| SpoolError::Io)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_data()) {
        let _ = fs::remove_file(temporary);
        let _ = error;
        return Err(SpoolError::Io);
    }
    if fs::rename(temporary, destination).is_err() {
        let _ = fs::remove_file(temporary);
        return Err(SpoolError::Io);
    }
    Ok(())
}
