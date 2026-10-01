//! Provider-independent, fail-closed file/path policy and content redaction.
mod policy;
mod redaction;

pub use policy::{
    Decision, FilePolicy, PolicyError, PolicyResult, Reason, Source, MAX_ANALYSIS_CONTENT_BYTES,
    MAX_CONTENT_BYTES, MAX_IGNORE_BYTES, POLICY_VERSION,
};
pub use redaction::{
    secret_class_for_key, Finding, RedactionEngine, RedactionError, RedactionResult, SecretClass,
    StructuredResult, MAX_REDACTION_INPUT_BYTES, REDACTION_RULES_VERSION,
};
