use mochi_privacy::{secret_class_for_key, RedactionEngine, RedactionError};
use std::path::{Path, PathBuf};

pub const MAX_TEXT_BYTES: usize = 8 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedText {
    pub value: String,
    pub redaction_count: u32,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SanitizeError {
    UnsafeInput,
}

pub trait ContentSanitizer: Send + Sync {
    fn sanitize_text(&self, value: &str) -> Result<SanitizedText, SanitizeError>;
}

/// Adapter-level path masking and text bounds; secret rules live only in `mochi-privacy`.
pub struct CaptureSanitizer {
    redactor: RedactionEngine,
    home: Option<PathBuf>,
    project_root: PathBuf,
    temp_root: PathBuf,
}

impl CaptureSanitizer {
    pub fn new(project_root: &Path) -> Result<Self, RedactionError> {
        Ok(Self {
            redactor: RedactionEngine::new()?,
            home: std::env::var_os("HOME").map(PathBuf::from),
            project_root: project_root.to_path_buf(),
            temp_root: std::env::temp_dir(),
        })
    }

    fn replace_literal(value: String, needle: &Path, replacement: &str, count: &mut u32) -> String {
        let needle = needle.to_string_lossy();
        if needle.is_empty() {
            return value;
        }
        let occurrences = value.matches(needle.as_ref()).count();
        *count = count.saturating_add(occurrences as u32);
        value.replace(needle.as_ref(), replacement)
    }
}

impl ContentSanitizer for CaptureSanitizer {
    fn sanitize_text(&self, value: &str) -> Result<SanitizedText, SanitizeError> {
        let redacted = self
            .redactor
            .redact_text(value)
            .map_err(|_| SanitizeError::UnsafeInput)?;
        let mut redaction_count = redacted
            .findings
            .iter()
            .fold(0u32, |total, item| total.saturating_add(item.count));
        let mut sanitized = redacted.text;
        sanitized = Self::replace_literal(
            sanitized,
            &self.project_root,
            "[PROJECT_ROOT]",
            &mut redaction_count,
        );
        if let Some(home) = &self.home {
            sanitized = Self::replace_literal(sanitized, home, "[HOME]", &mut redaction_count);
        }
        sanitized =
            Self::replace_literal(sanitized, &self.temp_root, "[TEMP]", &mut redaction_count);

        let truncated = sanitized.len() > MAX_TEXT_BYTES;
        if truncated {
            let mut end = MAX_TEXT_BYTES;
            while end > 0 && !sanitized.is_char_boundary(end) {
                end -= 1;
            }
            sanitized.truncate(end);
            sanitized.push_str("[TRUNCATED]");
        }
        Ok(SanitizedText {
            value: sanitized,
            redaction_count,
            truncated,
        })
    }
}

/// Compatibility alias for existing Brief 01 fixture call sites; no prototype rules remain.
pub type PrototypeSanitizer = CaptureSanitizer;

pub fn is_sensitive_key(key: &str) -> bool {
    secret_class_for_key(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::{is_sensitive_key, CaptureSanitizer, ContentSanitizer, MAX_TEXT_BYTES};
    use std::path::Path;

    #[test]
    fn removes_synthetic_credentials_before_truncation() {
        let sanitizer = CaptureSanitizer::new(Path::new("/approved/project")).expect("patterns");
        let value = format!(
            "{}\nAuthorization: Bearer abcdefghijklmnop\nOPENAI_API_KEY=sk-test_12345678901234567890\n/approved/project/src/main.rs",
            "x".repeat(MAX_TEXT_BYTES + 100)
        );
        let result = sanitizer.sanitize_text(&value).expect("sanitizes");
        assert!(result.truncated);
        assert!(result.redaction_count >= 3);
        assert!(!result.value.contains("abcdefghijklmnop"));
        assert!(!result.value.contains("sk-test"));
        assert!(!result.value.contains("/approved/project"));
    }

    #[test]
    fn recognizes_sensitive_field_names_without_broad_key_substrings() {
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("database-password"));
        assert!(!is_sensitive_key("monkey"));
        assert!(!is_sensitive_key("primaryKeyColumn"));
        assert!(!is_sensitive_key("keyboardLayout"));
    }
}
