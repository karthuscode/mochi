use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DETECTION_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationProvider {
    Codex,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientSurface {
    Cli,
    Desktop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupport {
    Supported,
    Partial,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    VerifiedExact,
    HistoricalBaseline,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub support: CapabilitySupport,
    pub evidence_level: EvidenceLevel,
    pub baseline_version: Option<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitySet {
    pub session_lifecycle: CapabilityEvidence,
    pub user_prompt: CapabilityEvidence,
    pub agent_response: CapabilityEvidence,
    pub tool_activity: CapabilityEvidence,
    pub commands: CapabilityEvidence,
    pub file_activity: CapabilityEvidence,
    pub permissions: CapabilityEvidence,
    pub interrupts: CapabilityEvidence,
    pub git_supplementation: CapabilityEvidence,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionState {
    Known,
    Unreadable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Verified,
    PartiallyVerified,
    RequiresValidation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    Arm64,
    X86_64,
    Universal,
    Other,
    Unknown,
}

impl Architecture {
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "aarch64" => Self::Arm64,
            "x86_64" => Self::X86_64,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Installation {
    /// Native-only local path. It must not cross into learning or outbound data.
    pub location: PathBuf,
    pub version: Option<String>,
    pub version_state: VersionState,
    pub build: Option<String>,
    pub bundle_identifier: Option<String>,
    pub architecture: Architecture,
    pub compatibility: Compatibility,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigurationState {
    Absent,
    PresentWithoutMochi,
    MochiConfigured,
    MochiCommandMismatch,
    Malformed,
    Unreadable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationReport {
    pub state: ConfigurationState,
    pub scopes_checked: u32,
    pub scopes_with_hooks: u32,
    pub unrelated_hooks_present: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustState {
    Trusted,
    Untrusted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperState {
    Missing,
    Present,
    NotExecutable,
    VersionMismatch,
    ArchitectureMismatch,
    Unreadable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelperReport {
    pub state: HelperState,
    pub version: Option<String>,
    pub architecture: Architecture,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Readiness {
    NotInstalled,
    Detected,
    AmbiguousInstallations,
    RequiresMochiHook,
    RequiresCodexConfiguration,
    RequiresTrust,
    RequiresCompatibilityValidation,
    Ready,
    Degraded,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionError {
    PermissionDenied,
    VersionParseFailed,
    ExecutableFailed,
    ExecutableTimedOut,
    MalformedConfiguration,
    UnsupportedPlatform,
    MetadataUnreadable,
    OutputLimitExceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SurfaceDetection {
    pub surface: ClientSurface,
    pub installations: Vec<Installation>,
    pub capabilities: CapabilitySet,
    pub configuration: ConfigurationReport,
    pub helper: HelperReport,
    pub trust: TrustState,
    pub readiness: Readiness,
    pub errors: Vec<DetectionError>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegrationDetection {
    pub schema_version: u8,
    pub provider: IntegrationProvider,
    pub surfaces: Vec<SurfaceDetection>,
}
