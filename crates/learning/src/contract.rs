use crate::{
    Evidence, LearningAnalysisInput, LearningError, LearningResult, Question, hash, parse_question,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Observed,
    Inferred,
    Unknown,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Claim {
    pub text: String,
    pub confidence: Confidence,
    pub references: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Reconstruction {
    pub overview: Vec<Claim>,
    pub built: Vec<Claim>,
    pub changed: Vec<Claim>,
    pub decisions: Vec<Claim>,
    pub failures: Vec<Claim>,
    pub unresolved: Vec<Claim>,
}
impl Reconstruction {
    pub fn sections(&self) -> [(&str, &Vec<Claim>); 6] {
        [
            ("Session overview", &self.overview),
            ("Requested and observed work", &self.built),
            ("What changed", &self.changed),
            ("Decisions and alternatives", &self.decisions),
            ("Failures and fixes", &self.failures),
            ("What remains unknown", &self.unresolved),
        ]
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConceptProposal {
    pub key: String,
    pub title: String,
    pub definition: String,
    pub relevance: f64,
    pub importance: f64,
    pub impact: f64,
    pub references: Vec<String>,
    pub code_reference: Option<String>,
    pub code_quote: Option<String>,
    pub why_it_works: String,
    pub why_it_matters: String,
    pub misconception: String,
    pub question: Question,
    pub delayed_variant: Question,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generation {
    pub schema_version: u16,
    pub reconstruction: Reconstruction,
    pub concepts: Vec<ConceptProposal>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Components {
    pub novelty: f64,
    pub relevance: f64,
    pub importance: f64,
    pub impact: f64,
    pub weakness: f64,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Priority {
    pub concept: ConceptProposal,
    pub components: Components,
    pub selection_score: f64,
    pub question_id: Uuid,
    pub delayed_question_id: Uuid,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningDocument {
    pub schema_version: u16,
    pub id: Uuid,
    pub input: LearningAnalysisInput,
    pub input_hash: String,
    pub model: String,
    pub contract_version: String,
    pub reconstruction: Reconstruction,
    pub priorities: Vec<Priority>,
}
fn references(refs: &[String], evidence: &[Evidence], required: bool) -> LearningResult<()> {
    if refs.len() > 8
        || required && refs.is_empty()
        || refs.iter().collect::<HashSet<_>>().len() != refs.len()
        || refs.iter().any(|r| !evidence.iter().any(|e| &e.id == r))
    {
        return Err(LearningError::InvalidReference);
    }
    Ok(())
}
fn bounded(s: &str, min: usize, max: usize) -> bool {
    s.trim().len() >= min
        && s.len() <= max
        && !s
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
}
fn canonical_key(key: &str) -> String {
    match key {
        "js.async-await" => "javascript.async-await",
        "js.strict-equality" => "javascript.strict-equality",
        "unit-testing" => "testing.assertions",
        _ => key,
    }
    .into()
}
fn valid_key(key: &str) -> bool {
    key.len() <= 80
        && key.split('.').count() >= 2
        && key.split('.').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
fn sanitized_value(
    value: &mut serde_json::Value,
    sanitizer: &crate::bundle::TextSanitizer,
) -> LearningResult<()> {
    match value {
        serde_json::Value::String(s) => *s = sanitizer.text(s, "")?,
        serde_json::Value::Array(xs) => {
            for x in xs {
                sanitized_value(x, sanitizer)?;
            }
        }
        serde_json::Value::Object(xs) => {
            for x in xs.values_mut() {
                sanitized_value(x, sanitizer)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn validate_generation(
    bytes: &[u8],
    input: LearningAnalysisInput,
    exposed: &BTreeSet<String>,
    incorrect: &BTreeSet<String>,
    model: &str,
) -> LearningResult<LearningDocument> {
    let mut value = mochi_privacy::parse_bounded_json(bytes, 256 * 1024)
        .map_err(|_| LearningError::InvalidOutput)?;
    sanitized_value(&mut value, &crate::bundle::TextSanitizer::new()?)?;
    let generated: Generation =
        serde_json::from_value(value).map_err(|_| LearningError::InvalidOutput)?;
    if generated.schema_version != 1 || generated.concepts.len() > 5 {
        return Err(LearningError::InvalidOutput);
    }
    for (_, claims) in generated.reconstruction.sections() {
        if claims.is_empty() || claims.len() > 8 {
            return Err(LearningError::InvalidOutput);
        }
        for claim in claims {
            if !bounded(&claim.text, 3, 1800) {
                return Err(LearningError::InvalidOutput);
            }
            references(
                &claim.references,
                &input.evidence,
                claim.confidence != Confidence::Unknown,
            )?;
            let lower = claim.text.to_lowercase();
            if claim.confidence == Confidence::Observed
                && (lower.contains("tests passed")
                    || lower.contains("test passed")
                    || lower.contains("tests succeeded"))
                && !claim.references.iter().any(|r| {
                    input
                        .evidence
                        .iter()
                        .any(|e| &e.id == r && e.verified_success)
                })
            {
                return Err(LearningError::InvalidReference);
            }
        }
    }
    let mut priorities = Vec::new();
    let mut keys = HashSet::new();
    let id = Uuid::new_v4();
    for mut concept in generated.concepts {
        concept.key = canonical_key(&concept.key);
        if !valid_key(&concept.key)
            || !keys.insert(concept.key.clone())
            || !bounded(&concept.title, 3, 100)
            || !bounded(&concept.definition, 10, 1800)
            || !bounded(&concept.why_it_works, 10, 3000)
            || !bounded(&concept.why_it_matters, 10, 2000)
            || !bounded(&concept.misconception, 10, 1000)
        {
            return Err(LearningError::InvalidOutput);
        }
        references(&concept.references, &input.evidence, true)?;
        for x in [concept.relevance, concept.importance, concept.impact] {
            if !x.is_finite() || !(0.0..=1.0).contains(&x) {
                return Err(LearningError::InvalidOutput);
            }
        }
        let words = [
            &concept.definition,
            &concept.why_it_works,
            &concept.why_it_matters,
            &concept.misconception,
        ]
        .iter()
        .map(|s| s.split_whitespace().count())
        .sum::<usize>();
        if !(150..=250).contains(&words) {
            return Err(LearningError::InvalidOutput);
        }
        match (&concept.code_reference, &concept.code_quote) {
            (Some(reference), Some(quote)) => {
                let evidence = input
                    .evidence
                    .iter()
                    .find(|e| &e.id == reference && e.kind == "code")
                    .ok_or(LearningError::InvalidReference)?;
                if !bounded(quote, 1, 2200)
                    || !evidence.text.contains(quote)
                    || !concept.references.contains(reference)
                {
                    return Err(LearningError::InvalidReference);
                }
            }
            (None, None) => {}
            _ => return Err(LearningError::InvalidReference),
        }
        parse_question(&concept.question)?;
        parse_question(&concept.delayed_variant)?;
        if concept.question.prompt.trim() == concept.delayed_variant.prompt.trim() {
            return Err(LearningError::InvalidQuestion);
        }
        let seen = exposed.contains(&concept.key);
        let c = Components {
            novelty: if seen { 0.8 } else { 1.0 },
            relevance: concept.relevance,
            importance: concept.importance,
            impact: concept.impact,
            weakness: if incorrect.contains(&concept.key) {
                1.0
            } else if seen {
                0.7
            } else {
                0.8
            },
        };
        let score = ((c.novelty * 0.20
            + c.relevance * 0.25
            + c.importance * 0.20
            + c.impact * 0.20
            + c.weakness * 0.15)
            * 10_000.0)
            .round()
            / 10_000.0;
        if c.relevance >= 0.5 && score >= 0.45 {
            let question_id = stable_id(&format!("{id}|{}|initial", concept.key));
            let delayed_question_id = stable_id(&format!("{id}|{}|delayed", concept.key));
            priorities.push(Priority {
                concept,
                components: c,
                selection_score: score,
                question_id,
                delayed_question_id,
            });
        }
    }
    priorities.sort_by(|a, b| {
        b.selection_score
            .total_cmp(&a.selection_score)
            .then(a.concept.key.cmp(&b.concept.key))
    });
    priorities.truncate(3);
    if !priorities.is_empty() && !priorities.iter().any(|p| p.concept.question.kind != "mcq") {
        return Err(LearningError::InvalidQuestion);
    }
    let doc = LearningDocument {
        schema_version: 1,
        id,
        input_hash: input.input_hash()?,
        input,
        model: model.into(),
        contract_version: "internal-explanation-v1".into(),
        reconstruction: generated.reconstruction,
        priorities,
    };
    if serde_json::to_vec(&doc)
        .map_err(|_| LearningError::InvalidOutput)?
        .len()
        > 384 * 1024
    {
        return Err(LearningError::TooLarge);
    }
    Ok(doc)
}
pub fn stable_id(text: &str) -> Uuid {
    let h = hash(text);
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap_or(0);
    }
    Uuid::from_bytes(bytes)
}

impl LearningDocument {
    pub fn validate(&self) -> LearningResult<()> {
        if self.id.is_nil()
            || self.schema_version != 1
            || self.contract_version != "internal-explanation-v1"
            || self.model != crate::MODEL
            || self.priorities.len() > 3
            || self.input.schema_version != 1
            || self.input.input_revision == 0
            || self.input.policy_revision == 0
            || self.input.file_policy_fingerprint.len() != 64
            || !self
                .input
                .file_policy_fingerprint
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || !self.input.useful()
            || self.input.input_hash()? != self.input_hash
            || self.input.evidence.len() > 70
            || self.input.omissions.len() > 20
            || self.input.registry.len() > 32
        {
            return Err(LearningError::InvalidOutput);
        }
        let sanitizer = crate::bundle::TextSanitizer::new()?;
        let mut ids = HashSet::new();
        for evidence in &self.input.evidence {
            if evidence.id.len() > 100
                || !ids.insert(&evidence.id)
                || evidence.text.len() > 3000
                || hash(&evidence.text) != evidence.excerpt_hash
                || mochi_domain::UtcTimestamp::parse(&evidence.captured_at).is_err()
                || sanitizer.text(&evidence.text, "")? != evidence.text
                || evidence.path.as_ref().is_some_and(|p| {
                    p.is_empty()
                        || p.starts_with('/')
                        || p.contains('\\')
                        || p.split('/').any(|part| part == "..")
                })
            {
                return Err(LearningError::InvalidReference);
            }
        }
        let generated = Generation {
            schema_version: 1,
            reconstruction: self.reconstruction.clone(),
            concepts: self.priorities.iter().map(|p| p.concept.clone()).collect(),
        };
        let exposed = self
            .priorities
            .iter()
            .filter(|p| p.components.novelty == 0.8)
            .map(|p| p.concept.key.clone())
            .collect();
        let incorrect = self
            .priorities
            .iter()
            .filter(|p| p.components.weakness == 1.0)
            .map(|p| p.concept.key.clone())
            .collect();
        let expected = validate_generation(
            &serde_json::to_vec(&generated).map_err(|_| LearningError::InvalidOutput)?,
            self.input.clone(),
            &exposed,
            &incorrect,
            &self.model,
        )?;
        if expected.priorities.len() != self.priorities.len() {
            return Err(LearningError::InvalidOutput);
        }
        for (actual, expected) in self.priorities.iter().zip(expected.priorities) {
            if actual.concept != expected.concept
                || actual.components != expected.components
                || actual.selection_score != expected.selection_score
                || actual.question_id
                    != stable_id(&format!("{}|{}|initial", self.id, actual.concept.key))
                || actual.delayed_question_id
                    != stable_id(&format!("{}|{}|delayed", self.id, actual.concept.key))
            {
                return Err(LearningError::InvalidOutput);
            }
        }
        Ok(())
    }
}
