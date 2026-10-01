use crate::model::*;
use crate::process::run_bounded;
use crate::IntegrationDetectionService;
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VERIFIED_CLI_VERSION: &str = "0.151.0";
pub const VERIFIED_DESKTOP_VERSION: &str = "26.908.40834";
const CODEX_BUNDLE_ID: &str = "com.openai.codex";
const MAX_INSTALLATIONS_PER_SURFACE: usize = 32;

#[derive(Clone, Debug)]
pub struct CodexDetectorOptions {
    pub path: OsString,
    pub codex_home: Option<PathBuf>,
    pub project_roots: Vec<PathBuf>,
    pub desktop_search_roots: Vec<PathBuf>,
    pub helper_path: PathBuf,
    pub expected_helper_version: String,
    pub expected_architecture: Architecture,
    pub command_timeout: Duration,
    /// Codex currently exposes trust through its interactive UI, not a safe file contract.
    pub known_trust: Option<TrustState>,
}

impl CodexDetectorOptions {
    pub fn system(helper_path: PathBuf) -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut desktop_search_roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = &home {
            desktop_search_roots.push(home.join("Applications"));
        }
        Self {
            path: std::env::var_os("PATH").unwrap_or_default(),
            codex_home: home.map(|path| path.join(".codex")),
            project_roots: Vec::new(),
            desktop_search_roots,
            helper_path,
            expected_helper_version: env!("CARGO_PKG_VERSION").to_owned(),
            expected_architecture: Architecture::current(),
            command_timeout: Duration::from_secs(2),
            known_trust: None,
        }
    }
}

pub struct CodexDetector {
    options: CodexDetectorOptions,
}

impl CodexDetector {
    pub fn new(options: CodexDetectorOptions) -> Self {
        Self { options }
    }

    fn detect_cli(
        &self,
        configuration: &ConfigurationReport,
        helper: &HelperReport,
    ) -> SurfaceDetection {
        let mut errors = configuration_error(configuration)
            .into_iter()
            .collect::<Vec<_>>();
        let mut installations = Vec::new();
        let mut candidates = cli_candidates(&self.options.path);
        if candidates.len() > MAX_INSTALLATIONS_PER_SURFACE {
            candidates.truncate(MAX_INSTALLATIONS_PER_SURFACE);
            errors.push(DetectionError::OutputLimitExceeded);
        }
        for path in candidates {
            let architecture = executable_architecture(&path).unwrap_or(Architecture::Unknown);
            let (version, version_state) =
                match run_bounded(&path, &["--version"], self.options.command_timeout) {
                    Ok(output) => match parse_cli_version(&output) {
                        Some(version) => (Some(version), VersionState::Known),
                        None => {
                            errors.push(DetectionError::VersionParseFailed);
                            (None, VersionState::Unreadable)
                        }
                    },
                    Err(error) => {
                        errors.push(error);
                        (None, VersionState::Unreadable)
                    }
                };
            let compatibility = if version.as_deref() == Some(VERIFIED_CLI_VERSION) {
                Compatibility::Verified
            } else {
                Compatibility::RequiresValidation
            };
            installations.push(Installation {
                location: path,
                version,
                version_state,
                build: None,
                bundle_identifier: None,
                architecture,
                compatibility,
            });
        }
        let capabilities = capability_matrix(
            ClientSurface::Cli,
            installations.len() == 1 && installations[0].compatibility == Compatibility::Verified,
        );
        let trust = self.options.known_trust.unwrap_or(TrustState::Unknown);
        let readiness = derive_readiness(
            ClientSurface::Cli,
            &installations,
            configuration,
            helper,
            trust,
        );
        SurfaceDetection {
            surface: ClientSurface::Cli,
            installations,
            capabilities,
            configuration: configuration.clone(),
            helper: helper.clone(),
            trust,
            readiness,
            errors: dedupe_errors(errors),
        }
    }

    fn detect_desktop(
        &self,
        configuration: &ConfigurationReport,
        helper: &HelperReport,
    ) -> SurfaceDetection {
        let mut errors = configuration_error(configuration)
            .into_iter()
            .collect::<Vec<_>>();
        let mut installations = Vec::new();
        #[cfg(target_os = "macos")]
        {
            for application in desktop_candidates(&self.options.desktop_search_roots) {
                if installations.len() >= MAX_INSTALLATIONS_PER_SURFACE {
                    errors.push(DetectionError::OutputLimitExceeded);
                    break;
                }
                match read_bundle_metadata(&application, self.options.command_timeout) {
                    Ok(Some(metadata)) if metadata.bundle_identifier == CODEX_BUNDLE_ID => {
                        let compatibility = if metadata.version == VERIFIED_DESKTOP_VERSION
                            || metadata.build.as_deref() == Some(VERIFIED_DESKTOP_VERSION)
                        {
                            Compatibility::PartiallyVerified
                        } else {
                            Compatibility::RequiresValidation
                        };
                        installations.push(Installation {
                            location: application,
                            version: Some(metadata.version),
                            version_state: VersionState::Known,
                            build: metadata.build,
                            bundle_identifier: Some(metadata.bundle_identifier),
                            architecture: metadata.architecture,
                            compatibility,
                        });
                    }
                    Ok(_) => {}
                    Err(error) => errors.push(error),
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        errors.push(DetectionError::UnsupportedPlatform);
        installations.sort_by(|left, right| left.location.cmp(&right.location));
        let capabilities = capability_matrix(
            ClientSurface::Desktop,
            installations.len() == 1
                && installations[0].compatibility == Compatibility::PartiallyVerified,
        );
        let trust = self.options.known_trust.unwrap_or(TrustState::Unknown);
        let readiness = derive_readiness(
            ClientSurface::Desktop,
            &installations,
            configuration,
            helper,
            trust,
        );
        SurfaceDetection {
            surface: ClientSurface::Desktop,
            installations,
            capabilities,
            configuration: configuration.clone(),
            helper: helper.clone(),
            trust,
            readiness,
            errors: dedupe_errors(errors),
        }
    }
}

impl IntegrationDetectionService for CodexDetector {
    fn detect(&self) -> IntegrationDetection {
        let configuration = inspect_configuration(
            self.options.codex_home.as_deref(),
            &self.options.project_roots,
            &self.options.helper_path,
        );
        let helper = inspect_helper(
            &self.options.helper_path,
            &self.options.expected_helper_version,
            self.options.expected_architecture,
            self.options.command_timeout,
        );
        IntegrationDetection {
            schema_version: DETECTION_SCHEMA_VERSION,
            provider: IntegrationProvider::Codex,
            surfaces: vec![
                self.detect_cli(&configuration, &helper),
                self.detect_desktop(&configuration, &helper),
            ],
        }
    }
}

pub fn parse_cli_version(output: &str) -> Option<String> {
    let trimmed = output.trim();
    let mut matches = trimmed
        .split_whitespace()
        .filter(|part| is_dotted_version(part));
    let version = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(version.to_owned())
}

fn is_dotted_version(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.len() <= 4
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|value| value.is_ascii_digit()))
}

pub(crate) fn cli_candidates(path: &OsString) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for directory in std::env::split_paths(path) {
        let candidate = directory.join("codex");
        if !candidate.is_file() || !is_executable(&candidate) {
            continue;
        }
        let identity = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.insert(identity) {
            result.push(candidate);
        }
    }
    result
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

fn inspect_helper(
    path: &Path,
    expected_version: &str,
    expected_architecture: Architecture,
    timeout: Duration,
) -> HelperReport {
    if !path.exists() {
        return helper(HelperState::Missing, None, Architecture::Unknown);
    }
    if !path.is_file() {
        return helper(HelperState::Unreadable, None, Architecture::Unknown);
    }
    if !is_executable(path) {
        return helper(HelperState::NotExecutable, None, Architecture::Unknown);
    }
    let architecture = executable_architecture(path).unwrap_or(Architecture::Unknown);
    if architecture != Architecture::Unknown
        && architecture != Architecture::Universal
        && architecture != expected_architecture
    {
        return helper(HelperState::ArchitectureMismatch, None, architecture);
    }
    match run_bounded(path, &["version"], timeout)
        .ok()
        .and_then(|output| parse_helper_version(&output))
    {
        Some(version) if version == expected_version => {
            helper(HelperState::Present, Some(version), architecture)
        }
        Some(version) => helper(HelperState::VersionMismatch, Some(version), architecture),
        None => helper(HelperState::Unreadable, None, architecture),
    }
}

fn helper(state: HelperState, version: Option<String>, architecture: Architecture) -> HelperReport {
    HelperReport {
        state,
        version,
        architecture,
    }
}

fn parse_helper_version(output: &str) -> Option<String> {
    let mut fields = output.split_whitespace();
    if fields.next()? != "mochi-hook" {
        return None;
    }
    let version = fields.next()?;
    if fields.next().is_some() || !is_dotted_version(version) {
        return None;
    }
    Some(version.to_owned())
}

fn inspect_configuration(
    codex_home: Option<&Path>,
    project_roots: &[PathBuf],
    helper_path: &Path,
) -> ConfigurationReport {
    let mut files = Vec::new();
    if let Some(home) = codex_home {
        files.push(home.join("hooks.json"));
    }
    for root in project_roots {
        files.push(root.join(".codex/hooks.json"));
    }
    let mut report = ConfigurationReport {
        state: ConfigurationState::Absent,
        scopes_checked: files.len() as u32,
        scopes_with_hooks: 0,
        unrelated_hooks_present: false,
    };
    let mut mochi_exact = false;
    let mut mochi_other = false;
    for path in files {
        if !path.exists() {
            continue;
        }
        let bytes = match crate::hooks::read_config(&path) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(()) => {
                report.state = ConfigurationState::Unreadable;
                return report;
            }
        };
        let value = match crate::hooks::parse_config(&bytes) {
            Ok(value) => value,
            Err(()) => {
                report.state = ConfigurationState::Malformed;
                return report;
            }
        };
        let summary = summarize_hooks(&value, helper_path);
        report.scopes_with_hooks += u32::from(summary.has_entries);
        mochi_exact |= summary.exact;
        mochi_other |= summary.other_mochi;
        report.unrelated_hooks_present |= summary.unrelated;
    }
    report.state = if mochi_exact {
        ConfigurationState::MochiConfigured
    } else if mochi_other {
        ConfigurationState::MochiCommandMismatch
    } else if report.scopes_with_hooks > 0 {
        ConfigurationState::PresentWithoutMochi
    } else {
        ConfigurationState::Absent
    };
    report
}

#[derive(Default)]
struct HookSummary {
    has_entries: bool,
    exact: bool,
    other_mochi: bool,
    unrelated: bool,
}

fn summarize_hooks(value: &Value, expected_helper: &Path) -> HookSummary {
    let mut summary = HookSummary::default();
    let mut covered = std::collections::BTreeSet::new();
    if let Some(events) = value.get("hooks").and_then(Value::as_object) {
        for (event, groups) in events {
            for group in groups.as_array().into_iter().flatten() {
                let unrestricted = group.get("matcher").is_none_or(|v| v.as_str() == Some(""));
                for handler in group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    summary.has_entries = true;
                    let command = handler.get("command").and_then(Value::as_str).unwrap_or("");
                    let exact = handler.get("type").and_then(Value::as_str) == Some("command")
                        && crate::hooks::valid_capture_command(command, expected_helper);
                    if exact && unrestricted {
                        covered.insert(event.as_str());
                    }
                    if exact
                        || crate::hooks::literal_words(command).is_some_and(|words| {
                            words.first().is_some_and(|word| {
                                Path::new(word)
                                    .file_name()
                                    .is_some_and(|name| name == "mochi-hook")
                            })
                        })
                    {
                        summary.other_mochi = true;
                    } else {
                        summary.unrelated = true;
                    }
                }
            }
        }
    }
    summary.exact = crate::hooks::CAPTURE_EVENTS
        .iter()
        .all(|event| covered.contains(event));
    summary.other_mochi &= !summary.exact;
    summary
}

fn capability_matrix(surface: ClientSurface, exact_baseline: bool) -> CapabilitySet {
    let baseline = match surface {
        ClientSurface::Cli => VERIFIED_CLI_VERSION,
        ClientSurface::Desktop => VERIFIED_DESKTOP_VERSION,
    };
    let level = if exact_baseline {
        EvidenceLevel::VerifiedExact
    } else {
        EvidenceLevel::HistoricalBaseline
    };
    let item = |support, evidence_ids: &[&str]| CapabilityEvidence {
        support,
        evidence_level: level,
        baseline_version: Some(baseline.to_owned()),
        evidence_ids: evidence_ids
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    };
    let unknown = || item(CapabilitySupport::Unknown, &[]);
    match surface {
        ClientSurface::Cli => CapabilitySet {
            session_lifecycle: item(CapabilitySupport::Supported, &["H-OBS"]),
            user_prompt: item(CapabilitySupport::Supported, &["H-OBS"]),
            agent_response: item(CapabilitySupport::Supported, &["H-OBS"]),
            tool_activity: item(CapabilitySupport::Supported, &["H-OBS"]),
            commands: item(CapabilitySupport::Supported, &["H-OBS"]),
            file_activity: unknown(),
            permissions: item(CapabilitySupport::Supported, &["H-PERM"]),
            interrupts: item(CapabilitySupport::Supported, &["H-OBS"]),
            git_supplementation: item(CapabilitySupport::Supported, &["G-OBS"]),
        },
        ClientSurface::Desktop => CapabilitySet {
            session_lifecycle: item(CapabilitySupport::Supported, &["D-OBS"]),
            user_prompt: unknown(),
            agent_response: item(CapabilitySupport::Supported, &["D-OBS"]),
            tool_activity: item(CapabilitySupport::Supported, &["D-OBS"]),
            commands: item(CapabilitySupport::Supported, &["D-OBS"]),
            file_activity: unknown(),
            permissions: item(CapabilitySupport::Supported, &["D-OBS"]),
            interrupts: unknown(),
            git_supplementation: item(CapabilitySupport::Supported, &["G-OBS"]),
        },
    }
}

fn derive_readiness(
    surface: ClientSurface,
    installations: &[Installation],
    configuration: &ConfigurationReport,
    helper: &HelperReport,
    trust: TrustState,
) -> Readiness {
    if installations.is_empty() {
        return Readiness::NotInstalled;
    }
    if installations.len() > 1 {
        return Readiness::AmbiguousInstallations;
    }
    if installations[0].compatibility == Compatibility::RequiresValidation {
        return Readiness::RequiresCompatibilityValidation;
    }
    match helper.state {
        HelperState::Missing
        | HelperState::NotExecutable
        | HelperState::VersionMismatch
        | HelperState::ArchitectureMismatch
        | HelperState::Unreadable => return Readiness::RequiresMochiHook,
        HelperState::Present => {}
    }
    match configuration.state {
        ConfigurationState::Absent
        | ConfigurationState::PresentWithoutMochi
        | ConfigurationState::MochiCommandMismatch => {
            return Readiness::RequiresCodexConfiguration;
        }
        ConfigurationState::Malformed | ConfigurationState::Unreadable => return Readiness::Error,
        ConfigurationState::MochiConfigured => {}
    }
    match trust {
        TrustState::Unknown | TrustState::Untrusted => Readiness::RequiresTrust,
        TrustState::Trusted if surface == ClientSurface::Desktop => Readiness::Degraded,
        TrustState::Trusted => Readiness::Ready,
    }
}

fn dedupe_errors(errors: Vec<DetectionError>) -> Vec<DetectionError> {
    let mut output = Vec::new();
    for error in errors {
        if !output.contains(&error) {
            output.push(error);
        }
    }
    output
}

fn configuration_error(configuration: &ConfigurationReport) -> Option<DetectionError> {
    match configuration.state {
        ConfigurationState::Malformed => Some(DetectionError::MalformedConfiguration),
        ConfigurationState::Unreadable => Some(DetectionError::PermissionDenied),
        _ => None,
    }
}

fn executable_architecture(path: &Path) -> Option<Architecture> {
    let mut bytes = [0u8; 8];
    let mut file = fs::File::open(path).ok()?;
    if file.read_exact(&mut bytes).is_err() {
        return Some(Architecture::Unknown);
    }
    let magic = u32::from_be_bytes(bytes[0..4].try_into().ok()?);
    let little_magic = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
    if matches!(magic, 0xcafebabe | 0xcafebabf) || matches!(little_magic, 0xcafebabe | 0xcafebabf) {
        return Some(Architecture::Universal);
    }
    let little = matches!(little_magic, 0xfeedface | 0xfeedfacf);
    let big = matches!(magic, 0xfeedface | 0xfeedfacf);
    if !little && !big {
        return Some(Architecture::Unknown);
    }
    let cpu = if little {
        u32::from_le_bytes(bytes[4..8].try_into().ok()?)
    } else {
        u32::from_be_bytes(bytes[4..8].try_into().ok()?)
    };
    Some(match cpu {
        0x0100000c => Architecture::Arm64,
        0x01000007 => Architecture::X86_64,
        _ => Architecture::Other,
    })
}

#[cfg(target_os = "macos")]
fn desktop_candidates(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("app") {
                result.push(path);
            }
        }
    }
    result.sort();
    result
}

#[cfg(target_os = "macos")]
pub(crate) struct BundleMetadata {
    bundle_identifier: String,
    version: String,
    build: Option<String>,
    architecture: Architecture,
}

#[cfg(target_os = "macos")]
pub(crate) fn read_bundle_metadata(
    application: &Path,
    timeout: Duration,
) -> Result<Option<BundleMetadata>, DetectionError> {
    let plist = application.join("Contents/Info.plist");
    if !plist.is_file() {
        return Ok(None);
    }
    let output = run_bounded(
        Path::new("/usr/bin/plutil"),
        &[
            "-convert",
            "json",
            "-o",
            "-",
            plist.to_str().ok_or(DetectionError::MetadataUnreadable)?,
        ],
        timeout,
    )?;
    let value: Value =
        serde_json::from_str(&output).map_err(|_| DetectionError::MetadataUnreadable)?;
    let bundle_identifier = value
        .get("CFBundleIdentifier")
        .and_then(Value::as_str)
        .ok_or(DetectionError::MetadataUnreadable)?;
    if bundle_identifier != CODEX_BUNDLE_ID {
        return Ok(None);
    }
    let version = value
        .get("CFBundleShortVersionString")
        .or_else(|| value.get("CFBundleVersion"))
        .and_then(Value::as_str)
        .ok_or(DetectionError::MetadataUnreadable)?;
    let build = value
        .get("CFBundleVersion")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let executable = value
        .get("CFBundleExecutable")
        .and_then(Value::as_str)
        .map(|name| application.join("Contents/MacOS").join(name));
    let architecture = executable
        .as_deref()
        .and_then(executable_architecture)
        .unwrap_or(Architecture::Unknown);
    Ok(Some(BundleMetadata {
        bundle_identifier: bundle_identifier.to_owned(),
        version: version.to_owned(),
        build,
        architecture,
    }))
}
