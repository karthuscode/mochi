use regex::{Captures, Regex};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const REDACTION_RULES_VERSION: &str = "redaction-v1";
pub const MAX_REDACTION_INPUT_BYTES: usize = 1024 * 1024;
const MAX_DEPTH: usize = 32;
const MAX_ITEMS_PER_CONTAINER: usize = 256;
const MAX_TOTAL_NODES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SecretClass {
    OpenAiApiKey,
    GithubToken,
    AwsAccessKeyId,
    AwsSecretAccessKey,
    BearerToken,
    Jwt,
    DatabaseCredential,
    Password,
    PrivateKey,
    GenericSecret,
}

impl SecretClass {
    pub const fn marker(self) -> &'static str {
        match self {
            Self::OpenAiApiKey => "[REDACTED:OPENAI_API_KEY]",
            Self::GithubToken => "[REDACTED:GITHUB_TOKEN]",
            Self::AwsAccessKeyId => "[REDACTED:AWS_ACCESS_KEY_ID]",
            Self::AwsSecretAccessKey => "[REDACTED:AWS_SECRET_ACCESS_KEY]",
            Self::BearerToken => "[REDACTED:BEARER_TOKEN]",
            Self::Jwt => "[REDACTED:JWT]",
            Self::DatabaseCredential => "[REDACTED:DATABASE_CREDENTIAL]",
            Self::Password => "[REDACTED:PASSWORD]",
            Self::PrivateKey => "[REDACTED:PRIVATE_KEY]",
            Self::GenericSecret => "[REDACTED:GENERIC_SECRET]",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub class: SecretClass,
    pub count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactionResult {
    pub text: String,
    pub changed: bool,
    pub findings: Vec<Finding>,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructuredResult {
    pub value: Value,
    pub changed: bool,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RedactionError {
    PatternUnavailable,
    InputTooLarge,
    StructureTooDeep,
    StructureTooLarge,
    UnsafeKey,
    UnterminatedPrivateKey,
}

pub struct RedactionEngine {
    private_key: Regex,
    private_key_start: Regex,
    database_url: Regex,
    bearer: Regex,
    assignment: Regex,
    openai: Regex,
    github: Regex,
    aws_key_id: Regex,
    jwt: Regex,
}

impl RedactionEngine {
    pub fn new() -> Result<Self, RedactionError> {
        let compile = |pattern| Regex::new(pattern).map_err(|_| RedactionError::PatternUnavailable);
        Ok(Self {
            private_key: compile(
                r"(?s)-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----.*?-----END (?:[A-Z0-9 ]+ )?PRIVATE KEY-----",
            )?,
            private_key_start: compile(r"-----BEGIN (?:[A-Z0-9 ]+ )?PRIVATE KEY-----")?,
            database_url: compile(
                r#"(?i)\b(postgres(?:ql)?|mysql|mongodb(?:\+srv)?)://[^\s"'<>:@/]+:[^\s"'<>@/]+@[^\s"'<>]+"#,
            )?,
            bearer: compile(r"(?i)\b(Bearer\s+)([A-Za-z0-9._~+/=-]{8,})")?,
            assignment: compile(
                r#"(?i)\b((?:export\s+)?[A-Za-z_][A-Za-z0-9_-]*\s*[:=]\s*)(\"[^\"\r\n]*\"|'[^'\r\n]*'|[^\s,;\"']+)"#,
            )?,
            openai: compile(r"\b(?:sk|pk|rk)-[A-Za-z0-9_-]{12,}\b")?,
            github: compile(r"\b(?:gh[pousr]_[A-Za-z0-9_]{12,}|github_pat_[A-Za-z0-9_]{12,})\b")?,
            aws_key_id: compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b")?,
            jwt: compile(r"\beyJ[A-Za-z0-9_-]{7,}\.eyJ[A-Za-z0-9_-]{7,}\.[A-Za-z0-9_-]{8,}\b")?,
        })
    }

    pub fn redact_text(&self, input: &str) -> Result<RedactionResult, RedactionError> {
        if input.len() > MAX_REDACTION_INPUT_BYTES {
            return Err(RedactionError::InputTooLarge);
        }
        let mut findings = BTreeMap::new();
        let mut text = replace_fixed(
            &self.private_key,
            input,
            SecretClass::PrivateKey,
            &mut findings,
        );
        if self.private_key_start.is_match(&text) {
            return Err(RedactionError::UnterminatedPrivateKey);
        }
        text = self
            .database_url
            .replace_all(&text, |capture: &Captures<'_>| {
                add(&mut findings, SecretClass::DatabaseCredential);
                format!(
                    "{}://{}",
                    &capture[1],
                    SecretClass::DatabaseCredential.marker()
                )
            })
            .into_owned();
        text = self
            .bearer
            .replace_all(&text, |capture: &Captures<'_>| {
                add(&mut findings, SecretClass::BearerToken);
                format!("{}{}", &capture[1], SecretClass::BearerToken.marker())
            })
            .into_owned();
        text = replace_fixed(
            &self.openai,
            &text,
            SecretClass::OpenAiApiKey,
            &mut findings,
        );
        text = replace_fixed(&self.github, &text, SecretClass::GithubToken, &mut findings);
        text = replace_fixed(
            &self.aws_key_id,
            &text,
            SecretClass::AwsAccessKeyId,
            &mut findings,
        );
        text = replace_fixed(&self.jwt, &text, SecretClass::Jwt, &mut findings);
        text = self
            .assignment
            .replace_all(&text, |capture: &Captures<'_>| {
                let prefix = &capture[1];
                let value = &capture[2];
                let key = prefix
                    .trim_end_matches([':', '=', ' ', '\t'])
                    .split_whitespace()
                    .last()
                    .unwrap_or("");
                let Some(class) = secret_class_for_key(key) else {
                    return capture[0].to_owned();
                };
                let unquoted = value.trim_matches(['\"', '\'']);
                if unquoted.starts_with("[REDACTED:")
                    || (class == SecretClass::BearerToken
                        && unquoted.eq_ignore_ascii_case("Bearer"))
                {
                    return capture[0].to_owned();
                }
                if class == SecretClass::DatabaseCredential
                    && unquoted.ends_with(SecretClass::DatabaseCredential.marker())
                    && [
                        "postgres://",
                        "postgresql://",
                        "mysql://",
                        "mongodb://",
                        "mongodb+srv://",
                    ]
                    .iter()
                    .any(|scheme| unquoted.to_ascii_lowercase().starts_with(scheme))
                {
                    return capture[0].to_owned();
                }
                add(&mut findings, class);
                let marker = class.marker();
                if value.starts_with('\"') {
                    format!("{prefix}\"{marker}\"")
                } else if value.starts_with('\'') {
                    format!("{prefix}'{marker}'")
                } else {
                    format!("{prefix}{marker}")
                }
            })
            .into_owned();
        if text.len() > MAX_REDACTION_INPUT_BYTES {
            return Err(RedactionError::InputTooLarge);
        }
        Ok(RedactionResult {
            changed: text != input,
            text,
            findings: to_findings(findings),
            truncated: false,
        })
    }

    pub fn redact_value(&self, input: &Value) -> Result<StructuredResult, RedactionError> {
        let mut budget = Budget { bytes: 0, nodes: 0 };
        let mut findings = BTreeMap::new();
        let value = self.walk(input, 0, &mut budget, &mut findings)?;
        Ok(StructuredResult {
            changed: value != *input,
            value,
            findings: to_findings(findings),
        })
    }

    fn walk(
        &self,
        value: &Value,
        depth: usize,
        budget: &mut Budget,
        findings: &mut BTreeMap<SecretClass, u32>,
    ) -> Result<Value, RedactionError> {
        if depth > MAX_DEPTH {
            return Err(RedactionError::StructureTooDeep);
        }
        budget.nodes += 1;
        if budget.nodes > MAX_TOTAL_NODES {
            return Err(RedactionError::StructureTooLarge);
        }
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(value.clone()),
            Value::String(text) => {
                budget.add_bytes(text.len())?;
                let trimmed = text.trim();
                if (trimmed.starts_with('{') && trimmed.ends_with('}'))
                    || (trimmed.starts_with('[') && trimmed.ends_with(']'))
                {
                    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                        let nested = self.walk(&parsed, depth + 1, budget, findings)?;
                        if nested != parsed {
                            return serde_json::to_string(&nested)
                                .map(Value::String)
                                .map_err(|_| RedactionError::StructureTooLarge);
                        }
                        return Ok(value.clone());
                    }
                }
                let result = self.redact_text(text)?;
                merge(findings, &result.findings);
                Ok(Value::String(result.text))
            }
            Value::Array(values) => {
                if values.len() > MAX_ITEMS_PER_CONTAINER {
                    return Err(RedactionError::StructureTooLarge);
                }
                values
                    .iter()
                    .map(|item| self.walk(item, depth + 1, budget, findings))
                    .collect()
            }
            Value::Object(values) => {
                if values.len() > MAX_ITEMS_PER_CONTAINER {
                    return Err(RedactionError::StructureTooLarge);
                }
                let mut output = Map::new();
                for (key, item) in values {
                    budget.add_bytes(key.len())?;
                    if self.redact_text(key)?.changed {
                        return Err(RedactionError::UnsafeKey);
                    }
                    if let Some(class) = secret_class_for_key(key) {
                        self.charge_opaque(item, depth + 1, budget)?;
                        if item.as_str() != Some(class.marker()) {
                            add(findings, class);
                        }
                        output.insert(key.clone(), Value::String(class.marker().to_owned()));
                    } else {
                        output.insert(key.clone(), self.walk(item, depth + 1, budget, findings)?);
                    }
                }
                Ok(Value::Object(output))
            }
        }
    }

    fn charge_opaque(
        &self,
        value: &Value,
        depth: usize,
        budget: &mut Budget,
    ) -> Result<(), RedactionError> {
        if depth > MAX_DEPTH {
            return Err(RedactionError::StructureTooDeep);
        }
        budget.nodes += 1;
        if budget.nodes > MAX_TOTAL_NODES {
            return Err(RedactionError::StructureTooLarge);
        }
        match value {
            Value::String(text) => budget.add_bytes(text.len()),
            Value::Array(items) => {
                if items.len() > MAX_ITEMS_PER_CONTAINER {
                    return Err(RedactionError::StructureTooLarge);
                }
                for item in items {
                    self.charge_opaque(item, depth + 1, budget)?;
                }
                Ok(())
            }
            Value::Object(items) => {
                if items.len() > MAX_ITEMS_PER_CONTAINER {
                    return Err(RedactionError::StructureTooLarge);
                }
                for (key, item) in items {
                    budget.add_bytes(key.len())?;
                    self.charge_opaque(item, depth + 1, budget)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[derive(Default)]
struct Budget {
    bytes: usize,
    nodes: usize,
}
impl Budget {
    fn add_bytes(&mut self, count: usize) -> Result<(), RedactionError> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or(RedactionError::InputTooLarge)?;
        if self.bytes > MAX_REDACTION_INPUT_BYTES {
            return Err(RedactionError::InputTooLarge);
        }
        Ok(())
    }
}

pub fn secret_class_for_key(key: &str) -> Option<SecretClass> {
    let normalized = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect::<String>();
    if normalized.ends_with("password") || normalized.ends_with("passwd") {
        Some(SecretClass::Password)
    } else if normalized == "openaikey"
        || (normalized.starts_with("openai") && normalized.ends_with("apikey"))
    {
        Some(SecretClass::OpenAiApiKey)
    } else if normalized.ends_with("awssecretaccesskey") {
        Some(SecretClass::AwsSecretAccessKey)
    } else if normalized.ends_with("awsaccesskeyid") {
        Some(SecretClass::AwsAccessKeyId)
    } else if normalized == "databaseurl" || normalized == "dburl" {
        Some(SecretClass::DatabaseCredential)
    } else if normalized == "authorization" {
        Some(SecretClass::BearerToken)
    } else if [
        "apikey",
        "token",
        "secret",
        "credential",
        "privatekey",
        "cookie",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
    {
        Some(SecretClass::GenericSecret)
    } else {
        None
    }
}

fn replace_fixed(
    regex: &Regex,
    input: &str,
    class: SecretClass,
    findings: &mut BTreeMap<SecretClass, u32>,
) -> String {
    regex
        .replace_all(input, |_capture: &Captures<'_>| {
            add(findings, class);
            class.marker().to_owned()
        })
        .into_owned()
}
fn add(findings: &mut BTreeMap<SecretClass, u32>, class: SecretClass) {
    let current = findings.get(&class).copied().unwrap_or(0);
    findings.insert(class, current.saturating_add(1));
}
fn merge(target: &mut BTreeMap<SecretClass, u32>, source: &[Finding]) {
    for finding in source {
        let current = target.get(&finding.class).copied().unwrap_or(0);
        target.insert(finding.class, current.saturating_add(finding.count));
    }
}
fn to_findings(findings: BTreeMap<SecretClass, u32>) -> Vec<Finding> {
    findings
        .into_iter()
        .map(|(class, count)| Finding { class, count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Instant;

    fn engine() -> RedactionEngine {
        RedactionEngine::new().expect("patterns")
    }

    #[test]
    fn known_tokens_aws_bearer_and_jwt_are_removed() {
        let input = concat!(
            "sk-synthetic-example-value-123\n",
            "ghp_SyntheticTokenValue123456789\n",
            "AKIAABCDEFGHIJKLMNOP\n",
            "Authorization: Bearer synthetic-token-value\n",
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJzeW50aGV0aWMifQ.c2lnbmF0dXJlMTIz\n"
        );
        let result = engine().redact_text(input).expect("redact");
        for fragment in [
            "synthetic-example",
            "SyntheticTokenValue",
            "AKIAABCDEFGHIJKLMNOP",
            "synthetic-token-value",
            "eyJhbGciOiJIUzI1NiJ9",
        ] {
            assert!(!result.text.contains(fragment), "known-token canary leaked");
        }
        assert!(result.text.contains(SecretClass::OpenAiApiKey.marker()));
        assert!(result.text.contains(SecretClass::GithubToken.marker()));
        assert!(result.text.contains(SecretClass::AwsAccessKeyId.marker()));
        assert!(result.text.contains(SecretClass::BearerToken.marker()));
        assert!(result.text.contains(SecretClass::Jwt.marker()));
    }

    #[test]
    fn assignments_and_database_urls_preserve_safe_context() {
        let input = concat!(
            "export OPENAI_API_KEY='sk-synthetic-example-value-123'\n",
            "AWS_SECRET_ACCESS_KEY=syntheticAwsSecretValue123\n",
            "password=synthetic-pass\nPASSWORD: synthetic-upper\n",
            "db_password=\"synthetic quoted password\"\n",
            "SECRET_TOKEN: synthetic-generic\n",
            "DATABASE_URL=postgres://alice:synthetic-db-pass@db.example.invalid/app\n",
            "mysql://bob:synthetic-mysql@localhost/db\n",
            "mongodb://carol:synthetic-mongo@localhost/db\n",
            "mongodb+srv://dave:synthetic-srv@localhost/db\n"
        );
        let result = engine().redact_text(input).expect("redact");
        for fragment in [
            "synthetic-example",
            "syntheticAws",
            "synthetic-pass",
            "synthetic-upper",
            "synthetic quoted",
            "synthetic-generic",
            "synthetic-db-pass",
            "synthetic-mysql",
            "synthetic-mongo",
            "synthetic-srv",
        ] {
            assert!(!result.text.contains(fragment), "assignment canary leaked");
        }
        assert!(result
            .text
            .contains("OPENAI_API_KEY='[REDACTED:OPENAI_API_KEY]'"));
        assert!(result.text.contains("PASSWORD: [REDACTED:PASSWORD]"));
        assert!(result
            .text
            .contains("postgres://[REDACTED:DATABASE_CREDENTIAL]"));
        assert!(result
            .text
            .contains("mongodb+srv://[REDACTED:DATABASE_CREDENTIAL]"));
        assert_eq!(
            engine().redact_text(&result.text).expect("second").text,
            result.text
        );
    }

    #[test]
    fn private_key_block_is_complete_and_unterminated_block_fails() {
        for label in ["", "RSA ", "EC ", "OPENSSH "] {
            let input = format!("before\n-----BEGIN {label}PRIVATE KEY-----\nsynthetic-body-line-a\nsynthetic-body-line-b\n-----END {label}PRIVATE KEY-----\nafter");
            let result = engine().redact_text(&input).expect("redact");
            assert_eq!(result.text, "before\n[REDACTED:PRIVATE_KEY]\nafter");
        }
        assert_eq!(
            engine().redact_text("-----BEGIN PRIVATE KEY-----\nsynthetic-body"),
            Err(RedactionError::UnterminatedPrivateKey)
        );
    }

    #[test]
    fn nested_values_use_key_context_and_preserve_shape() {
        let input = json!({"username":"alice", "password":"synthetic-pass", "nested":[{"client_secret":"synthetic-client", "token":"synthetic-token", "keyboardLayout":"us"}]});
        let result = engine().redact_value(&input).expect("redact");
        assert_eq!(result.value["username"], "alice");
        assert_eq!(result.value["password"], SecretClass::Password.marker());
        assert_eq!(
            result.value["nested"][0]["client_secret"],
            SecretClass::GenericSecret.marker()
        );
        assert_eq!(
            result.value["nested"][0]["token"],
            SecretClass::GenericSecret.marker()
        );
        assert_eq!(result.value["nested"][0]["keyboardLayout"], "us");
        assert_eq!(
            engine().redact_value(&result.value).expect("second").value,
            result.value
        );
    }

    #[test]
    fn ordinary_developer_values_are_not_redacted() {
        let input = concat!(
            "550e8400-e29b-41d4-a716-446655440000\n",
            "0123456789abcdef0123456789abcdef01234567\n",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
            "sha512-ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/==\n",
            "primaryKeyColumn = 42\nkeyboardLayout: US\n",
            "https://example.invalid/docs\nversion=1.2.3\ncolor: #aabbcc\n",
            "const testId = 'synthetic-id';"
        );
        let result = engine().redact_text(input).expect("redact");
        assert_eq!(result.text, input);
        assert!(!result.changed);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn oversized_and_unsafe_structures_fail_closed() {
        let oversized = "x".repeat(MAX_REDACTION_INPUT_BYTES + 1);
        assert_eq!(
            engine().redact_text(&oversized),
            Err(RedactionError::InputTooLarge)
        );
        assert_eq!(
            engine().redact_value(&json!({"password":oversized})),
            Err(RedactionError::InputTooLarge)
        );
        let unsafe_key = json!({"sk-synthetic-example-value-123":"safe"});
        assert_eq!(
            engine().redact_value(&unsafe_key),
            Err(RedactionError::UnsafeKey)
        );
    }

    #[test]
    fn representative_redaction_cost() {
        let engine = engine();
        for (name, input) in [
            ("1k", "let value = 42;\n".repeat(64)),
            ("10k", format!("{}\npassword=synthetic-pass", "let value = 42;\n".repeat(640))),
            ("64k", "let value = 42;\n".repeat(4096)),
            ("secrets", "OPENAI_API_KEY=sk-synthetic-example-value-123\nAuthorization: Bearer synthetic-token-value\n".repeat(100)),
        ] {
            let mut timings = Vec::new();
            for _ in 0..25 {
                let start = Instant::now();
                assert!(engine.redact_text(&input).is_ok());
                timings.push(start.elapsed().as_micros());
            }
            timings.sort_unstable();
            eprintln!("redaction {name}: median={}us p95={}us", timings[12], timings[23]);
        }
        let nested = json!({"items": (0..100).map(|index| json!({"name":format!("item-{index}"),"client_secret":"synthetic-secret"})).collect::<Vec<_>>()});
        let start = Instant::now();
        assert!(engine.redact_value(&nested).is_ok());
        eprintln!("redaction nested-100: {}us", start.elapsed().as_micros());
    }
}
