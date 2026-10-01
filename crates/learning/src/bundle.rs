use crate::{LearningError, LearningResult};
use mochi_domain::{
    CodingSession, CompletionStatus, GitContext, Project, SessionEventData, SessionId,
};
use mochi_privacy::{FilePolicy, RedactionEngine, Source};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MAX_INPUT_BYTES: usize = 128 * 1024;
pub const MAX_ESTIMATED_TOKENS: usize = 24_000;
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub path: Option<String>,
    pub captured_at: String,
    pub first_line: Option<u32>,
    pub excerpt_hash: String,
    pub truncated: bool,
    pub verified_success: bool,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningAnalysisInput {
    pub schema_version: u16,
    pub session_id: SessionId,
    pub input_revision: u64,
    pub policy_revision: u64,
    pub file_policy_fingerprint: String,
    pub project_alias: String,
    pub coverage: String,
    pub attribution_warning: String,
    pub stop_reason: String,
    pub language: String,
    pub evidence: Vec<Evidence>,
    pub omissions: Vec<String>,
    pub registry: Vec<String>,
}
pub fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub(crate) struct TextSanitizer {
    paths: Regex,
    redactor: RedactionEngine,
}
impl TextSanitizer {
    pub(crate) fn new() -> LearningResult<Self> {
        Ok(Self{
  paths:Regex::new(r#"(?m)(?P<prefix>^|[\s\"'`(=:])(?:/[A-Za-z_.~][^\s\"'`<>),;]*|[A-Za-z]:\\[^\s\"'`<>),;]*)"#).map_err(|_|LearningError::Sanitization)?,
  redactor:RedactionEngine::new().map_err(|_|LearningError::Sanitization)?,
 })
    }
    pub(crate) fn text(&self, text: &str, root: &str) -> LearningResult<String> {
        if text.len() > 384 * 1024 {
            return Err(LearningError::TooLarge);
        }
        let text = if root.is_empty() {
            text.to_owned()
        } else {
            text.replace(root, "[project]")
        };
        let text = self.paths.replace_all(&text, "${prefix}[LOCAL_PATH]");
        self.redactor
            .redact_text(&text)
            .map(|r| r.text)
            .map_err(|_| LearningError::Sanitization)
    }
}
pub fn sanitize(text: &str, root: &str) -> LearningResult<String> {
    TextSanitizer::new()?.text(text, root)
}

fn excerpt(text: &str, limit: usize) -> (String, bool) {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), end < text.len())
}
impl LearningAnalysisInput {
    pub fn canonical_json(&self) -> LearningResult<String> {
        let json = serde_json::to_string(self).map_err(|_| LearningError::InvalidInput)?;
        if json.len() > MAX_INPUT_BYTES || json.len().div_ceil(3) > MAX_ESTIMATED_TOKENS {
            return Err(LearningError::TooLarge);
        }
        Ok(json)
    }
    pub fn input_hash(&self) -> LearningResult<String> {
        Ok(hash(&self.canonical_json()?))
    }
    pub fn useful(&self) -> bool {
        self.evidence.iter().any(|e| {
            matches!(
                e.kind.as_str(),
                "code" | "technical_action" | "observed_result"
            ) && e.text.len() >= 20
        }) && self.evidence.iter().map(|e| e.text.len()).sum::<usize>() >= 80
    }
}
pub const REGISTRY: &[&str] = &[
    "javascript.async-await",
    "javascript.strict-equality",
    "javascript.array-transformation",
    "python.functions",
    "testing.assertions",
    "testing.boundary-cases",
    "concurrency.race-condition",
    "react.state",
    "react.context",
    "security.input-validation",
    "security.secret-management",
    "design.separation-of-concerns",
    "algorithms.accumulation",
    "git.dirty-working-tree",
];
pub fn prepare_input(
    session: &CodingSession,
    project: &Project,
    revision: u64,
    policy_revision: u64,
    stop_reason: &str,
    policy: &FilePolicy,
) -> LearningResult<LearningAnalysisInput> {
    if revision == 0
        || policy_revision == 0
        || session.data().project_id != project.id
        || session.data().ended_at.is_none()
    {
        return Err(LearningError::InvalidInput);
    }
    let root = &project.root_path;
    let sanitizer = TextSanitizer::new()?;
    let paths = Regex::new(r#"(?:\[PROJECT_ROOT\]/)?([A-Za-z0-9_./-]+\.[A-Za-z0-9_-]+)"#)
        .map_err(|_| LearningError::Sanitization)?;
    let mut evidence = Vec::new();
    let mut omissions=vec!["Provider-reported intent does not prove execution or understanding. Missing results remain unknown.".into()];
    let mut omitted_events = 0;
    for event in &session.data().events {
        let (kind, text, success) = match &event.event {
            SessionEventData::UserPrompt(v) => ("reported_intent", v.text.clone(), false),
            SessionEventData::AgentMessage(_) => {
                omissions.push("Agent prose omitted because embedded file content has no recheckable path provenance.".into());
                continue;
            }
            SessionEventData::ToolStarted(v) => (
                "technical_action",
                format!(
                    "Tool requested: {}. {}",
                    v.tool_kind,
                    v.summary.as_deref().unwrap_or("Input unavailable.")
                ),
                false,
            ),
            SessionEventData::ToolCompleted(v) => (
                "observed_result",
                format!(
                    "Tool status: {:?}. {}",
                    v.status, "Response content omitted; file provenance cannot be rechecked."
                ),
                false,
            ),
            SessionEventData::CommandExecuted(v) => ("technical_action", v.display.clone(), false),
            SessionEventData::CommandResult(v) => (
                "observed_result",
                format!(
                    "Exit code: {:?}. {}",
                    v.exit_code, "Output content omitted; file provenance cannot be rechecked."
                ),
                false,
            ),
            SessionEventData::TestResult(v) => (
                "observed_result",
                format!(
                    "Test status: {:?}; passed {:?}; failed {:?}. {}",
                    v.status,
                    v.passed_count,
                    v.failed_count,
                    v.summary.as_deref().unwrap_or("Details unavailable.")
                ),
                v.status == CompletionStatus::Succeeded,
            ),
            SessionEventData::ErrorEncountered(v) => ("error", v.summary.clone(), false),
            SessionEventData::CaptureGap(_) => (
                "gap",
                "Some activity was not captured. Do not infer missing outcomes.".into(),
                false,
            ),
            _ => continue,
        };
        if paths.captures_iter(&text).any(|c| {
            !policy
                .evaluate(Path::new(&c[1]), None, Source::Analysis)
                .content_allowed()
        }) {
            omissions.push("Activity referring to an excluded file was omitted.".into());
            continue;
        }
        let (text, truncated) = excerpt(&sanitizer.text(&text, root)?, 1200);
        evidence.push(Evidence {
            id: format!("event:{}", event.id),
            kind: kind.into(),
            excerpt_hash: hash(&text),
            text,
            path: None,
            captured_at: event.received_at.as_str().into(),
            first_line: None,
            truncated: truncated || event.sensitivity.truncated,
            verified_success: success,
        });
        if evidence.len() > 50 {
            evidence.remove(0);
            omitted_events += 1;
        }
    }
    if omitted_events > 0 {
        omissions.push(format!(
            "{omitted_events} earlier activity records omitted to keep the bundle bounded."
        ));
    }
    let snapshots = match &session.data().git_context {
        GitContext::Available { before, after } => {
            let mut v = vec![("before", before.as_ref())];
            if let Some(after) = after {
                v.push(("after", after.as_ref()));
            }
            v
        }
        GitContext::FinalOnly { after, .. } => {
            omissions.push(
                "No baseline snapshot exists; the final snapshot does not prove a session delta."
                    .into(),
            );
            vec![("after", after.as_ref())]
        }
        GitContext::Unavailable { .. } => {
            omissions.push("Git code context unavailable.".into());
            vec![]
        }
    };
    let mut code_count = 0;
    for (role, snapshot) in snapshots {
        for file in &snapshot.files {
            let Some(content) = &file.content else {
                omissions.push("A code file's content was unavailable or omitted.".into());
                continue;
            };
            if !policy
                .evaluate(
                    Path::new(&file.path),
                    Some(content.len() as u64),
                    Source::Analysis,
                )
                .content_allowed()
            {
                omissions.push("A file was excluded by current analysis policy.".into());
                continue;
            }
            if code_count >= 20 {
                continue;
            }
            code_count += 1;
            let lines = content.lines().take(80).collect::<Vec<_>>().join("\n");
            let (text, truncated) = excerpt(&sanitizer.text(&lines, root)?, 2200);
            let path = sanitizer.text(&file.path, root)?;
            let id = format!(
                "code:{role}:{}",
                hash(&format!(
                    "{}|{}|{}",
                    path,
                    snapshot.captured_at.as_str(),
                    hash(&text)
                ))
            );
            evidence.push(Evidence {
                id,
                kind: "code".into(),
                excerpt_hash: hash(&text),
                text,
                path: Some(path),
                captured_at: snapshot.captured_at.as_str().into(),
                first_line: Some(1),
                truncated: truncated || file.truncated || content.lines().count() > 80,
                verified_success: false,
            });
        }
    }
    omissions.truncate(16);
    omissions.sort();
    omissions.dedup();
    let mut input=LearningAnalysisInput{schema_version:1,session_id:session.data().id,input_revision:revision,policy_revision,file_policy_fingerprint:policy.fingerprint(),project_alias:sanitizer.text(&project.display_name,root)?,coverage:format!("{:?}",session.overall_completeness()).to_lowercase(),attribution_warning:"Changes can overlap other work. Neither a snapshot nor an agent statement proves authorship, test success or learner understanding.".into(),stop_reason:sanitizer.text(stop_reason,root)?,language:"English".into(),evidence,omissions,registry:REGISTRY.iter().map(|s|(*s).into()).collect()};
    while input.canonical_json().is_err() && input.evidence.len() > 1 {
        let position = input
            .evidence
            .iter()
            .position(|e| e.kind == "reported_result")
            .unwrap_or(0);
        input.evidence.remove(position);
        if !input
            .omissions
            .iter()
            .any(|x| x.starts_with("Bundle reduced"))
        {
            input
                .omissions
                .push("Bundle reduced to the provider input budget; some evidence omitted.".into());
        }
    }
    input.canonical_json()?;
    if !input.useful() {
        return Err(LearningError::InsufficientContext);
    }
    Ok(input)
}
