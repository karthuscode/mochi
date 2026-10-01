use regex::Regex;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const POLICY_VERSION: &str = "file-path-v1";
pub const MAX_CONTENT_BYTES: u64 = 64 * 1024;
pub const MAX_ANALYSIS_CONTENT_BYTES: u64 = 32 * 1024;
pub const MAX_IGNORE_BYTES: u64 = 16 * 1024;
const MAX_IGNORE_RULES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    Allow,
    MetadataOnly,
    Deny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Capture,
    Git,
    Analysis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reason {
    Allowed,
    BuiltinSecretFile,
    SensitiveDirectory,
    ToolDirectory,
    UserExcluded,
    LocalPolicyConfig,
    GeneratedArtifact,
    BinaryFile,
    DatabaseFile,
    Lockfile,
    FileTooLarge,
    OutsideProject,
    InvalidPath,
    Symlink,
    UnsafeFileType,
    PolicyUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyResult {
    pub decision: Decision,
    pub reason: Reason,
    /// Only project-relative paths are ever returned. Denied paths have no stored spelling.
    pub safe_path: Option<String>,
    pub policy_version: &'static str,
}

impl PolicyResult {
    fn new(decision: Decision, reason: Reason, path: Option<String>) -> Self {
        Self {
            decision,
            reason,
            safe_path: if decision == Decision::Deny {
                None
            } else {
                path
            },
            policy_version: POLICY_VERSION,
        }
    }

    pub fn content_allowed(&self) -> bool {
        self.decision == Decision::Allow
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyError {
    InvalidRoot,
    InvalidIgnore,
    IgnoreUnreadable,
}

pub struct FilePolicy {
    root: PathBuf,
    root_alias: PathBuf,
    ignore: Vec<Regex>,
}

impl FilePolicy {
    pub fn load(root: &Path) -> Result<Self, PolicyError> {
        let root_alias = root.to_path_buf();
        let root = root.canonicalize().map_err(|_| PolicyError::InvalidRoot)?;
        if !root.is_dir() {
            return Err(PolicyError::InvalidRoot);
        }
        let ignore_path = root.join(".mochiignore");
        let ignore = match fs::symlink_metadata(&ignore_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(_) => return Err(PolicyError::IgnoreUnreadable),
            Ok(metadata) => {
                if !metadata.is_file() || metadata.len() > MAX_IGNORE_BYTES {
                    return Err(PolicyError::InvalidIgnore);
                }
                let bytes = fs::read(&ignore_path).map_err(|_| PolicyError::IgnoreUnreadable)?;
                if bytes.len() as u64 > MAX_IGNORE_BYTES {
                    return Err(PolicyError::InvalidIgnore);
                }
                let text = std::str::from_utf8(&bytes).map_err(|_| PolicyError::InvalidIgnore)?;
                parse_ignore(text)?
            }
        };
        Ok(Self {
            root,
            root_alias,
            ignore,
        })
    }

    /// Opaque revision of interpreted rules; never exposes their original spelling.
    pub fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        digest.update(POLICY_VERSION.as_bytes());
        for rule in &self.ignore {
            digest.update((rule.as_str().len() as u64).to_be_bytes());
            digest.update(rule.as_str().as_bytes());
        }
        format!("{:x}", digest.finalize())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Evaluates a provider or Git path. `size` is optional for deleted and virtual paths.
    /// This method does not open file content. A caller must recheck before each actual read.
    pub fn evaluate(&self, candidate: &Path, size: Option<u64>, source: Source) -> PolicyResult {
        let relative = match normalize(&self.root, &self.root_alias, candidate) {
            Ok(path) => path,
            Err(reason) => return PolicyResult::new(Decision::Deny, reason, None),
        };
        let lower = relative.to_ascii_lowercase();
        let parts = lower.split('/').collect::<Vec<_>>();
        let name = parts.last().copied().unwrap_or_default();

        if parts.iter().any(|part| {
            matches!(
                *part,
                ".ssh" | ".aws" | ".azure" | ".gnupg" | ".kube" | ".codex" | ".git"
            )
        }) {
            return PolicyResult::new(Decision::Deny, Reason::SensitiveDirectory, None);
        }
        if name == ".mochiignore" {
            return PolicyResult::new(Decision::Deny, Reason::LocalPolicyConfig, None);
        }
        if name == ".env"
            || name.starts_with(".env.")
            || [".pem", ".key", ".p12", ".pfx", ".jks"]
                .iter()
                .any(|extension| name.ends_with(extension))
            || matches!(name, "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519")
            || name == "auth.json"
            || name.starts_with("credentials")
            || ["secrets.", "secret."]
                .iter()
                .any(|prefix| name.starts_with(prefix))
        {
            return PolicyResult::new(Decision::Deny, Reason::BuiltinSecretFile, None);
        }
        if [".sqlite", ".sqlite3", ".db", ".db-wal", ".db-shm"]
            .iter()
            .any(|extension| name.ends_with(extension))
        {
            return PolicyResult::new(Decision::Deny, Reason::DatabaseFile, None);
        }
        if let Err(reason) = self.check_filesystem(&relative) {
            return PolicyResult::new(Decision::Deny, reason, None);
        }
        if self.ignore.iter().any(|rule| rule.is_match(&relative)) {
            return PolicyResult::new(Decision::Deny, Reason::UserExcluded, None);
        }
        let metadata =
            |reason| PolicyResult::new(Decision::MetadataOnly, reason, Some(relative.clone()));
        if parts.iter().any(|part| {
            matches!(
                *part,
                "node_modules"
                    | "target"
                    | "dist"
                    | "build"
                    | "coverage"
                    | ".next"
                    | ".nuxt"
                    | ".cache"
                    | "cache"
                    | "vendor"
            )
        }) {
            return PolicyResult::new(Decision::Deny, Reason::ToolDirectory, None);
        }
        if parts.contains(&"generated") {
            return metadata(Reason::GeneratedArtifact);
        }
        if name.ends_with(".min.js") || name.ends_with(".map") || name.ends_with(".min.css") {
            return metadata(Reason::GeneratedArtifact);
        }
        if matches!(
            name,
            "pnpm-lock.yaml" | "package-lock.json" | "yarn.lock" | "cargo.lock"
        ) {
            return metadata(Reason::Lockfile);
        }
        if [
            ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".zip", ".tar", ".gz", ".7z",
            ".exe", ".dylib", ".so", ".o", ".a", ".mp3", ".mp4", ".mov", ".wav", ".woff", ".woff2",
            ".ttf", ".bin",
        ]
        .iter()
        .any(|extension| name.ends_with(extension))
        {
            return metadata(Reason::BinaryFile);
        }
        if size.is_some_and(|value| {
            value
                > if source == Source::Analysis {
                    MAX_ANALYSIS_CONTENT_BYTES
                } else {
                    MAX_CONTENT_BYTES
                }
        }) {
            return metadata(Reason::FileTooLarge);
        }
        PolicyResult::new(Decision::Allow, Reason::Allowed, Some(relative))
    }

    /// For a bounded sample already read under an ALLOW decision. No binary extraction.
    pub fn classify_sample(&self, candidate: &Path, sample: &[u8], source: Source) -> PolicyResult {
        let prior = self.evaluate(candidate, Some(sample.len() as u64), source);
        if prior.decision != Decision::Allow {
            return prior;
        }
        if sample.contains(&0) || std::str::from_utf8(sample).is_err() {
            return PolicyResult::new(Decision::MetadataOnly, Reason::BinaryFile, prior.safe_path);
        }
        prior
    }

    fn check_filesystem(&self, relative: &str) -> Result<(), Reason> {
        let mut current = self.root.clone();
        for part in relative.split('/') {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(Reason::Symlink);
                    }
                    if !metadata.is_file() && !metadata.is_dir() {
                        return Err(Reason::UnsafeFileType);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(_) => return Err(Reason::PolicyUnavailable),
            }
        }
        Ok(())
    }
}

fn normalize(root: &Path, root_alias: &Path, candidate: &Path) -> Result<String, Reason> {
    let text = candidate
        .to_str()
        .ok_or(Reason::InvalidPath)?
        .replace('\\', "/");
    if text.is_empty() || text.contains('\0') {
        return Err(Reason::InvalidPath);
    }
    let normalized = Path::new(&text);
    let relative = if normalized.is_absolute() {
        normalized
            .strip_prefix(root)
            .or_else(|_| normalized.strip_prefix(root_alias))
            .map_err(|_| Reason::OutsideProject)?
    } else {
        normalized
    };
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => {
                let part = part.to_str().ok_or(Reason::InvalidPath)?;
                if part.contains(':') {
                    return Err(Reason::InvalidPath);
                }
                parts.push(part);
            }
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(Reason::OutsideProject);
                }
            }
            _ => return Err(Reason::InvalidPath),
        }
    }
    if parts.is_empty() {
        return Err(Reason::InvalidPath);
    }
    Ok(parts.join("/"))
}

fn parse_ignore(text: &str) -> Result<Vec<Regex>, PolicyError> {
    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue; // Negation never widens V1 policy.
        }
        if rules.len() >= MAX_IGNORE_RULES || line.contains("..") || line.contains('\\') {
            return Err(PolicyError::InvalidIgnore);
        }
        let anchored = line.starts_with('/');
        let pattern = line.trim_start_matches('/');
        let directory = pattern.ends_with('/');
        let pattern = pattern.trim_end_matches('/');
        if pattern.is_empty()
            || pattern
                .split('/')
                .any(|part| part.is_empty() || part == ".")
            || pattern.chars().any(|ch| {
                !(ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '*' | '?'))
            })
        {
            return Err(PolicyError::InvalidIgnore);
        }
        let mut expression = if anchored || pattern.contains('/') {
            String::from("^")
        } else {
            String::from("(?:^|.*/)")
        };
        let chars = pattern.chars().collect::<Vec<_>>();
        let mut index = 0;
        while index < chars.len() {
            match chars[index] {
                '*' if chars.get(index + 1) == Some(&'*') => {
                    expression.push_str(".*");
                    index += 1;
                }
                '*' => expression.push_str("[^/]*"),
                '?' => expression.push_str("[^/]"),
                '/' => expression.push('/'),
                other => expression.push_str(&regex::escape(&other.to_string())),
            }
            index += 1;
        }
        if directory {
            expression.push_str("(?:/.*)?$");
        } else {
            expression.push_str("(?:$|/.*$)");
        }
        rules.push(Regex::new(&expression).map_err(|_| PolicyError::InvalidIgnore)?);
    }
    Ok(rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::TempDir;

    fn check(policy: &FilePolicy, path: &str, size: Option<u64>) -> (Decision, Reason) {
        let result = policy.evaluate(Path::new(path), size, Source::Git);
        (result.decision, result.reason)
    }

    #[test]
    fn secret_names_and_directories_are_denied_without_retaining_paths() {
        let root = TempDir::new().expect("root");
        let policy = FilePolicy::load(root.path()).expect("policy");
        for path in [
            ".env",
            ".ENV",
            ".Env",
            ".env.local",
            ".env.production",
            "keys/private.pem",
            "keys/private.key",
            "keys/private.p12",
            "keys/private.pfx",
            "keys/private.jks",
            "id_rsa",
            "id_dsa",
            "id_ecdsa",
            "id_ed25519",
            "credentials.json",
            "credentials.local",
            "credentialsBackup",
            "secrets.yml",
            "secret.toml",
            ".ssh/id_ed25519",
            ".aws/credentials",
            ".azure/token",
            ".gnupg/key",
            ".kube/config",
            ".git/config",
            "database.sqlite",
            "database.sqlite3",
            "app.db",
        ] {
            let result = policy.evaluate(Path::new(path), None, Source::Capture);
            assert_eq!(result.decision, Decision::Deny, "{path}");
            assert_eq!(result.safe_path, None, "{path}");
        }
        for path in [
            "src/auth.ts",
            "package.json",
            "Cargo.toml",
            "pyproject.toml",
            "requirements.txt",
            "src/secretary.ts",
        ] {
            assert_eq!(check(&policy, path, None).0, Decision::Allow, "{path}");
        }
    }

    #[test]
    fn traversal_and_outside_paths_fail_closed() {
        let root = TempDir::new().expect("root");
        let policy = FilePolicy::load(root.path()).expect("policy");
        for path in [
            "../x",
            "../../x",
            "nested/../../../x",
            "src/../../.ssh/id_rsa",
        ] {
            assert_eq!(check(&policy, path, None).0, Decision::Deny, "{path}");
        }
        assert_eq!(
            check(&policy, "src/../.env", None).1,
            Reason::BuiltinSecretFile
        );
        assert_eq!(
            check(&policy, "src\\..\\.ENV", None).1,
            Reason::BuiltinSecretFile
        );
        let outside = root.path().parent().expect("parent").join("outside.rs");
        assert_eq!(
            policy.evaluate(&outside, None, Source::Git).reason,
            Reason::OutsideProject
        );
        let inside = root.path().join("src/main.ts");
        assert_eq!(
            policy
                .evaluate(&inside, None, Source::Git)
                .safe_path
                .as_deref(),
            Some("src/main.ts")
        );
        assert_eq!(
            check(&policy, "missing/not-yet-created.ts", None).0,
            Decision::Allow
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlink_components_and_ignore_symlinks_fail_closed() {
        use std::os::unix::fs::symlink;
        let root = TempDir::new().expect("root");
        let outside = TempDir::new().expect("outside");
        symlink(outside.path(), root.path().join("link")).expect("link");
        let policy = FilePolicy::load(root.path()).expect("policy");
        assert_eq!(
            check(&policy, "link/private.txt", None),
            (Decision::Deny, Reason::Symlink)
        );
        symlink(
            outside.path().join("missing"),
            root.path().join(".mochiignore"),
        )
        .expect("ignore link");
        assert!(matches!(
            FilePolicy::load(root.path()),
            Err(PolicyError::InvalidIgnore)
        ));
    }

    #[test]
    fn user_ignore_is_monotonic_and_malformed_rules_fail_closed() {
        let root = TempDir::new().expect("root");
        fs::write(
            root.path().join(".mochiignore"),
            "private/\n*.secret\ninternal/config/*\n!.env\n",
        )
        .expect("ignore");
        let policy = FilePolicy::load(root.path()).expect("policy");
        for path in [
            "private/notes.ts",
            "src/password.secret",
            "internal/config/prod.ts",
        ] {
            assert_eq!(
                check(&policy, path, None),
                (Decision::Deny, Reason::UserExcluded),
                "{path}"
            );
        }
        assert_eq!(check(&policy, ".env", None).1, Reason::BuiltinSecretFile);
        assert_eq!(check(&policy, "src/main.ts", None).0, Decision::Allow);
        assert_eq!(
            check(&policy, ".mochiignore", None).1,
            Reason::LocalPolicyConfig
        );
        fs::write(root.path().join(".mochiignore"), "../../escape\n").expect("invalid ignore");
        assert!(matches!(
            FilePolicy::load(root.path()),
            Err(PolicyError::InvalidIgnore)
        ));
    }

    #[test]
    fn metadata_only_classes_and_size_boundaries() {
        let root = TempDir::new().expect("root");
        let policy = FilePolicy::load(root.path()).expect("policy");
        for path in [
            "node_modules/pkg/index.js",
            "target/debug/a",
            "dist/app.min.js",
            "build/out.js",
            "coverage/report.html",
            "cache/item.js",
            ".next/cache/a",
        ] {
            assert_eq!(check(&policy, path, None).0, Decision::Deny, "{path}");
        }
        for path in [
            "generated/types.ts",
            "src/app.min.js",
            "src/app.js.map",
            "image.png",
            "archive.zip",
            "pnpm-lock.yaml",
            "package-lock.json",
            "yarn.lock",
            "Cargo.lock",
        ] {
            assert_eq!(
                check(&policy, path, None).0,
                Decision::MetadataOnly,
                "{path}"
            );
        }
        for size in [MAX_CONTENT_BYTES - 1, MAX_CONTENT_BYTES] {
            assert_eq!(check(&policy, "src/main.ts", Some(size)).0, Decision::Allow);
        }
        assert_eq!(
            check(&policy, "src/main.ts", Some(MAX_CONTENT_BYTES + 1)),
            (Decision::MetadataOnly, Reason::FileTooLarge)
        );
        assert_eq!(
            policy
                .evaluate(
                    Path::new("src/main.ts"),
                    Some(MAX_ANALYSIS_CONTENT_BYTES + 1),
                    Source::Analysis
                )
                .decision,
            Decision::MetadataOnly
        );
        assert_eq!(
            policy
                .classify_sample(Path::new("src/data"), b"hello\0world", Source::Git)
                .reason,
            Reason::BinaryFile
        );
        assert_eq!(
            policy
                .classify_sample(Path::new("src/data"), b"\xff", Source::Git)
                .decision,
            Decision::MetadataOnly
        );
    }

    #[test]
    fn representative_evaluation_cost() {
        let root = TempDir::new().expect("root");
        fs::write(root.path().join(".mochiignore"), "private/\n*.secret\n").expect("ignore");
        let policy = FilePolicy::load(root.path()).expect("policy");
        for count in [1, 100, 1_000] {
            let start = Instant::now();
            for index in 0..count {
                let path = format!("src/mod{index}.rs");
                assert_eq!(check(&policy, &path, None).0, Decision::Allow);
            }
            eprintln!(
                "privacy policy: {count} paths with .mochiignore: {:?}",
                start.elapsed()
            );
        }
    }
}
