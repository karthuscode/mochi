use crate::{LearningError, LearningResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub text: String,
    pub feedback: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Question {
    pub kind: String,
    pub prompt: String,
    pub choices: Vec<Choice>,
    pub correct_choice: Option<String>,
    pub snippet: Option<String>,
    pub assumptions: Option<String>,
    pub expected: Option<String>,
    pub criteria: Vec<String>,
    pub blocking_misconceptions: Vec<String>,
    pub explanation: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Correct,
    Incorrect,
    Uncertain,
    Pending,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CriterionGrade {
    pub criterion: String,
    pub outcome: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdvisoryGrade {
    pub overall: Grade,
    pub confidence: f64,
    pub criteria: Vec<CriterionGrade>,
    pub blocking_misconception: bool,
    pub feedback: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feedback {
    pub criteria: Vec<CriterionGrade>,
    pub blocking_misconception: Option<bool>,
    pub grade: Grade,
    pub method: String,
    pub confidence: Option<f64>,
    pub text: String,
}
fn prediction(snippet: &str) -> LearningResult<String> {
    // A small authoring validator, not a JavaScript runtime. Never executes generated code.
    let pattern =
        regex::Regex::new(r"^console\.log\((-?[0-9]{1,4}) (\+|\*|===) (-?[0-9]{1,4})\);$")
            .map_err(|_| LearningError::InvalidQuestion)?;
    let c = pattern
        .captures(snippet)
        .ok_or(LearningError::InvalidQuestion)?;
    let a: i32 = c[1].parse().map_err(|_| LearningError::InvalidQuestion)?;
    let b: i32 = c[3].parse().map_err(|_| LearningError::InvalidQuestion)?;
    Ok(match &c[2] {
        "+" => (a + b).to_string(),
        "*" => (a * b).to_string(),
        "===" => (a == b).to_string(),
        _ => return Err(LearningError::InvalidQuestion),
    })
}
pub fn parse_question(q: &Question) -> LearningResult<()> {
    if q.prompt.trim().len() < 10
        || q.prompt.len() > 1500
        || q.explanation.trim().len() < 10
        || q.explanation.len() > 2000
        || q.blocking_misconceptions.len() > 4
        || q.blocking_misconceptions
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 300)
    {
        return Err(LearningError::InvalidQuestion);
    }
    match q.kind.as_str() {
        "mcq" => {
            if !(3..=4).contains(&q.choices.len())
                || q.correct_choice
                    .as_ref()
                    .is_none_or(|id| !q.choices.iter().any(|c| &c.id == id))
                || !q.criteria.is_empty()
                || q.snippet.is_some()
                || q.expected.is_some()
                || q.assumptions.is_some()
            {
                return Err(LearningError::InvalidQuestion);
            }
            let mut ids = HashSet::new();
            let mut labels = HashSet::new();
            for c in &q.choices {
                if c.id.is_empty()
                    || c.id.len() > 16
                    || !c.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    || !ids.insert(&c.id)
                    || !labels.insert(c.text.trim().to_lowercase())
                    || c.text.trim().is_empty()
                    || c.text.len() > 500
                    || c.feedback.trim().len() < 10
                    || c.feedback.len() > 1000
                {
                    return Err(LearningError::InvalidQuestion);
                }
            }
        }
        "predict_output" => {
            let snippet = q.snippet.as_deref().ok_or(LearningError::InvalidQuestion)?;
            if q.expected
                .as_ref()
                .is_none_or(|e| prediction(snippet).ok().as_ref() != Some(e))
                || q.assumptions
                    .as_ref()
                    .is_none_or(|a| !a.to_lowercase().contains("javascript") || a.len() > 400)
                || !q.choices.is_empty()
                || q.correct_choice.is_some()
                || !q.criteria.is_empty()
            {
                return Err(LearningError::InvalidQuestion);
            }
        }
        "explain_why" => {
            if !(2..=4).contains(&q.criteria.len())
                || q.criteria.iter().collect::<HashSet<_>>().len() != q.criteria.len()
                || q.criteria
                    .iter()
                    .any(|c| c.trim().len() < 5 || c.len() > 400)
                || !q.choices.is_empty()
                || q.correct_choice.is_some()
                || q.expected.is_some()
                || q.snippet.is_some()
                || q.assumptions.is_some()
            {
                return Err(LearningError::InvalidQuestion);
            }
        }
        _ => return Err(LearningError::InvalidQuestion),
    }
    Ok(())
}
pub fn grade_local(q: &Question, answer: &str) -> LearningResult<Feedback> {
    parse_question(q)?;
    if answer.trim().is_empty() || answer.len() > 16 * 1024 {
        return Err(LearningError::InvalidInput);
    }
    let correct=match q.kind.as_str(){"mcq"=>{if !q.choices.iter().any(|c|c.id==answer.trim()){return Err(LearningError::InvalidInput)}q.correct_choice.as_deref()==Some(answer.trim())},"predict_output"=>q.expected.as_deref()==Some(answer.trim()),"explain_why"=>return Ok(Feedback{criteria:vec![],blocking_misconception:None,grade:Grade::Pending,method:"ungraded".into(),confidence:None,text:"Saved locally. Advisory rubric grading requires a separate answer preview and remote approval.".into()}),_=>return Err(LearningError::InvalidQuestion)};
    let explanation = if q.kind == "mcq" {
        format!(
            "{} {}",
            q.choices
                .iter()
                .find(|c| c.id == answer.trim())
                .map(|c| c.feedback.as_str())
                .unwrap_or(""),
            q.explanation
        )
    } else {
        q.explanation.clone()
    };
    Ok(Feedback {
        criteria: vec![],
        blocking_misconception: None,
        grade: if correct {
            Grade::Correct
        } else {
            Grade::Incorrect
        },
        method: "local".into(),
        confidence: None,
        text: explanation,
    })
}
pub fn validate_grade(bytes: &[u8], q: &Question) -> LearningResult<Feedback> {
    let value = mochi_privacy::parse_bounded_json(bytes, 16 * 1024)
        .map_err(|_| LearningError::InvalidOutput)?;
    let mut g: AdvisoryGrade =
        serde_json::from_value(value).map_err(|_| LearningError::InvalidOutput)?;
    if q.kind != "explain_why"
        || g.criteria.len() != q.criteria.len()
        || g.criteria.iter().zip(&q.criteria).any(|(a, b)| {
            &a.criterion != b || !matches!(a.outcome.as_str(), "pass" | "fail" | "uncertain")
        })
        || !g.confidence.is_finite()
        || !(0.0..=1.0).contains(&g.confidence)
        || g.feedback.trim().len() < 5
        || g.feedback.len() > 3000
        || g.overall == Grade::Pending
    {
        return Err(LearningError::InvalidOutput);
    }
    let grade = if g.confidence < 0.8 || g.criteria.iter().any(|c| c.outcome == "uncertain") {
        Grade::Uncertain
    } else if g.criteria.iter().any(|c| c.outcome == "fail") || g.blocking_misconception {
        Grade::Incorrect
    } else if g.overall == Grade::Correct {
        Grade::Correct
    } else {
        Grade::Uncertain
    };
    g.feedback = crate::sanitize(&g.feedback, "")?;
    Ok(Feedback {
        criteria: g.criteria,
        blocking_misconception: Some(g.blocking_misconception),
        grade,
        method: "ai_advisory".into(),
        confidence: Some(g.confidence),
        text: g.feedback,
    })
}
