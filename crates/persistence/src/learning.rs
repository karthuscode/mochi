use crate::{SqliteStore, StorageError, StorageResult, error::map_sqlite};
use mochi_domain::SessionId;
use mochi_learning::{Feedback, Grade, LearningDocument, Question, grade_local, hash, sanitize};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisRun {
    pub id: Uuid,
    pub session_id: SessionId,
    pub input_revision: u64,
    pub policy_revision: u64,
    pub input_hash: String,
    pub request_hash: String,
    pub model: String,
    pub purpose: String,
    pub attempt_id: Option<Uuid>,
}
#[derive(Clone)]
pub struct LearningQuestion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub concept_key: String,
    pub question: Question,
    pub revealed: bool,
    pub session_id: SessionId,
    pub input_revision: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningAttempt {
    pub id: Uuid,
    pub question_id: Uuid,
    pub answer: String,
    pub assistance: String,
    pub solution_seen: bool,
    pub feedback: Feedback,
    pub submitted_at: String,
}
fn encode<T: Serialize>(v: &T) -> StorageResult<String> {
    serde_json::to_string(v).map_err(|_| StorageError::Serialization)
}
fn decode<T: serde::de::DeserializeOwned>(v: &str) -> StorageResult<T> {
    if v.len() > 384 * 1024 {
        return Err(StorageError::Corrupt);
    }
    let val = mochi_privacy::parse_bounded_json(v.as_bytes(), 384 * 1024)
        .map_err(|_| StorageError::Corrupt)?;
    serde_json::from_value(val).map_err(|_| StorageError::Corrupt)
}
fn integer(n: u64) -> StorageResult<i64> {
    i64::try_from(n).map_err(|_| StorageError::InvalidInput)
}
fn current(c: &Connection, run: &AnalysisRun) -> StorageResult<bool> {
    c.query_row("SELECT EXISTS(SELECT 1 FROM capture_episodes e JOIN projects p ON p.id=e.project_id JOIN sessions s ON s.id=e.id WHERE e.id=?1 AND e.state='finalized' AND e.revision=?2 AND p.deleted_at IS NULL AND p.policy_revision=?3)",params![run.session_id.to_string(),integer(run.input_revision)?,integer(run.policy_revision)?],|r|r.get(0)).map_err(map_sqlite)
}
fn running(c: &Connection, run: &AnalysisRun) -> StorageResult<bool> {
    c.query_row("SELECT EXISTS(SELECT 1 FROM analysis_runs WHERE id=?1 AND session_id=?2 AND input_revision=?3 AND policy_revision=?4 AND input_hash=?5 AND request_hash=?6 AND model=?7 AND purpose=?8 AND attempt_id IS ?9 AND status='running')",params![run.id.to_string(),run.session_id.to_string(),integer(run.input_revision)?,integer(run.policy_revision)?,run.input_hash,run.request_hash,run.model,run.purpose,run.attempt_id.map(|id|id.to_string())],|r|r.get(0)).map_err(map_sqlite)
}

impl SqliteStore {
    pub fn recover_analysis(&self) -> StorageResult<()> {
        self.lock()?
            .execute(
                "UPDATE analysis_runs SET status='cancelled',finished_at=?1 WHERE status='running'",
                [self.now()?],
            )
            .map_err(map_sqlite)?;
        Ok(())
    }
    pub fn begin_analysis(&self, run: &AnalysisRun) -> StorageResult<()> {
        if run.id.is_nil()
            || run.input_revision == 0
            || run.policy_revision == 0
            || run.model.len() > 80
            || !matches!(run.purpose.as_str(), "generation" | "grading")
            || [&run.input_hash, &run.request_hash]
                .iter()
                .any(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(StorageError::InvalidInput);
        }
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        if (run.purpose == "generation" && run.attempt_id.is_some())
            || (run.purpose == "grading" && run.attempt_id.is_none())
        {
            return Err(StorageError::InvalidInput);
        }
        if !current(&tx, run)? {
            return Err(StorageError::PolicyRejected);
        }
        if let Some(id) = run.attempt_id {
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM selfcheck_attempts a JOIN selfcheck_questions q ON q.id=a.question_id JOIN learning_documents d ON d.id=q.document_id WHERE a.id=?1 AND a.grade='pending' AND d.session_id=?2 AND d.input_revision=?3)",
                params![id.to_string(), run.session_id.to_string(), integer(run.input_revision)?], |r| r.get(0)
            ).map_err(map_sqlite)?;
            if !valid {
                return Err(StorageError::PolicyRejected);
            }
        }
        tx.execute("INSERT INTO analysis_runs(id,session_id,input_revision,policy_revision,input_hash,request_hash,model,purpose,status,created_at,attempt_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'running',?9,?10)",params![run.id.to_string(),run.session_id.to_string(),integer(run.input_revision)?,integer(run.policy_revision)?,run.input_hash,run.request_hash,run.model,run.purpose,self.now()?,run.attempt_id.map(|id|id.to_string())]).map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)
    }
    pub fn finish_analysis_failure(&self, id: Uuid, cancelled: bool) -> StorageResult<()> {
        self.lock()?.execute("UPDATE analysis_runs SET status=?2,finished_at=?3 WHERE id=?1 AND status='running'",params![id.to_string(),if cancelled{"cancelled"}else{"failed"},self.now()?]).map_err(map_sqlite)?;
        Ok(())
    }
    pub fn publish_learning(&self, run: &AnalysisRun, doc: &LearningDocument) -> StorageResult<()> {
        if run.purpose != "generation"
            || doc.input.session_id != run.session_id
            || doc.input.input_revision != run.input_revision
            || doc.input.policy_revision != run.policy_revision
            || doc.input_hash != run.input_hash
            || doc.model != run.model
            || doc.schema_version != 1
            || doc.priorities.len() > 3
            || doc
                .input
                .input_hash()
                .map_err(|_| StorageError::DomainValidation)?
                != doc.input_hash
        {
            return Err(StorageError::DomainValidation);
        }
        doc.validate().map_err(|_| StorageError::DomainValidation)?;
        let text = encode(doc)?;
        if text.len() > 384 * 1024
            || sanitize(&text, "").map_err(|_| StorageError::DomainValidation)? != text
        {
            return Err(StorageError::DomainValidation);
        }
        for p in &doc.priorities {
            mochi_learning::parse_question(&p.concept.question)
                .map_err(|_| StorageError::DomainValidation)?;
            mochi_learning::parse_question(&p.concept.delayed_variant)
                .map_err(|_| StorageError::DomainValidation)?;
        }
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        if !running(&tx, run)? || !current(&tx, run)? {
            return Err(StorageError::PolicyRejected);
        }
        tx.execute("INSERT INTO learning_documents(id,run_id,session_id,input_revision,input_hash,model,contract_version,document_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![doc.id.to_string(),run.id.to_string(),run.session_id.to_string(),integer(run.input_revision)?,run.input_hash,run.model,doc.contract_version,text,self.now()?]).map_err(map_sqlite)?;
        for (pos, p) in doc.priorities.iter().enumerate() {
            for (variant, id, q) in [
                ("initial", p.question_id, &p.concept.question),
                ("delayed", p.delayed_question_id, &p.concept.delayed_variant),
            ] {
                tx.execute("INSERT INTO selfcheck_questions(id,document_id,concept_key,variant,position,question_json) VALUES(?1,?2,?3,?4,?5,?6)",params![id.to_string(),doc.id.to_string(),p.concept.key,variant,pos as i64,encode(q)?]).map_err(map_sqlite)?;
            }
            tx.execute("INSERT INTO learning_exposures(document_id,concept_key,exposed_at) VALUES(?1,?2,?3)",params![doc.id.to_string(),p.concept.key,self.now()?]).map_err(map_sqlite)?;
        }
        tx.execute(
            "UPDATE analysis_runs SET status='published',finished_at=?2 WHERE id=?1",
            params![run.id.to_string(), self.now()?],
        )
        .map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)
    }
    pub fn learning_history(&self) -> StorageResult<(BTreeSet<String>, BTreeSet<String>)> {
        let c = self.lock()?;
        let mut stmt=c.prepare("SELECT DISTINCT concept_key FROM learning_exposures ORDER BY concept_key LIMIT 500").map_err(map_sqlite)?;
        let exposed = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(map_sqlite)?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(map_sqlite)?;
        let cutoff = super::connection::utc_millis(&self.now()?)? - 30 * 24 * 60 * 60 * 1000;
        let mut stmt=c.prepare("SELECT DISTINCT q.concept_key FROM selfcheck_attempts a JOIN selfcheck_questions q ON q.id=a.question_id WHERE a.grade='incorrect' AND a.assistance='independent' AND a.solution_seen=0 AND (julianday(a.submitted_at)-2440587.5)*86400000>=?1 ORDER BY q.concept_key LIMIT 500").map_err(map_sqlite)?;
        let incorrect = stmt
            .query_map([cutoff], |r| r.get::<_, String>(0))
            .map_err(map_sqlite)?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(map_sqlite)?;
        Ok((exposed, incorrect))
    }
    pub fn latest_learning(&self, id: SessionId) -> StorageResult<Option<LearningDocument>> {
        let text:Option<String>=self.lock()?.query_row("SELECT document_json FROM learning_documents WHERE session_id=?1 ORDER BY created_at DESC,id DESC LIMIT 1",[id.to_string()],|r|r.get(0)).optional().map_err(map_sqlite)?;
        text.map(|text| {
            let doc: LearningDocument = decode(&text)?;
            doc.validate().map_err(|_| StorageError::Corrupt)?;
            Ok(doc)
        })
        .transpose()
    }
    pub fn get_learning_question(&self, id: Uuid) -> StorageResult<Option<LearningQuestion>> {
        question(&*self.lock()?, id)
    }
    pub fn submit_selfcheck(
        &self,
        id: Uuid,
        question_id: Uuid,
        answer: &str,
        assistance: &str,
    ) -> StorageResult<LearningAttempt> {
        if id.is_nil()
            || answer.trim().is_empty()
            || answer.len() > 16 * 1024
            || !matches!(assistance, "independent" | "assisted" | "unknown")
        {
            return Err(StorageError::InvalidInput);
        }
        let safe = sanitize(answer, "").map_err(|_| StorageError::DomainValidation)?;
        let request_hash = hash(&format!("{}|{}|{}", question_id, safe, assistance));
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        if let Some(existing) = attempt(&tx, id)? {
            let saved: String = tx
                .query_row(
                    "SELECT request_hash FROM selfcheck_attempts WHERE id=?1",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .map_err(map_sqlite)?;
            if saved != request_hash {
                return Err(StorageError::Constraint);
            }
            return Ok(existing);
        }
        let q = question(&tx, question_id)?.ok_or(StorageError::InvalidInput)?;
        let fresh:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM capture_episodes WHERE id=?1 AND state='finalized' AND revision=?2)",params![q.session_id.to_string(),integer(q.input_revision)?],|r|r.get(0)).map_err(map_sqlite)?;
        if !fresh {
            return Err(StorageError::PolicyRejected);
        }
        let feedback =
            grade_local(&q.question, &safe).map_err(|_| StorageError::DomainValidation)?;
        let assistance = if q.revealed { "assisted" } else { assistance }.to_owned();
        let submitted_at = self.now()?;
        tx.execute("INSERT INTO selfcheck_attempts(id,question_id,answer,assistance,solution_seen,grade,feedback_json,submitted_at,request_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![id.to_string(),question_id.to_string(),safe,assistance,q.revealed,grade_text(&feedback.grade),encode(&feedback)?,submitted_at,request_hash]).map_err(map_sqlite)?;
        if feedback.grade != Grade::Pending {
            tx.execute(
                "INSERT OR IGNORE INTO selfcheck_reveals(question_id,revealed_at) VALUES(?1,?2)",
                params![question_id.to_string(), submitted_at],
            )
            .map_err(map_sqlite)?;
        }
        tx.commit().map_err(map_sqlite)?;
        Ok(LearningAttempt {
            id,
            question_id,
            answer: safe,
            assistance,
            solution_seen: q.revealed,
            feedback,
            submitted_at,
        })
    }
    pub fn reveal_selfcheck(&self, id: Uuid) -> StorageResult<String> {
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let q = question(&tx, id)?.ok_or(StorageError::InvalidInput)?;
        tx.execute(
            "INSERT OR IGNORE INTO selfcheck_reveals(question_id,revealed_at) VALUES(?1,?2)",
            params![id.to_string(), self.now()?],
        )
        .map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)?;
        let answer = q
            .question
            .correct_choice
            .as_ref()
            .and_then(|id| {
                q.question
                    .choices
                    .iter()
                    .find(|c| &c.id == id)
                    .map(|c| c.text.clone())
            })
            .or(q.question.expected)
            .unwrap_or_else(|| q.question.criteria.join("; "));
        Ok(format!("{answer}\n{}", q.question.explanation))
    }
    pub fn selfcheck_attempts(&self, id: Uuid) -> StorageResult<Vec<LearningAttempt>> {
        let c = self.lock()?;
        let mut stmt=c.prepare("SELECT id FROM selfcheck_attempts WHERE question_id=?1 ORDER BY submitted_at DESC,id DESC LIMIT 50").map_err(map_sqlite)?;
        let ids = stmt
            .query_map([id.to_string()], |r| r.get::<_, String>(0))
            .map_err(map_sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_sqlite)?;
        ids.iter()
            .map(|id| {
                attempt(&c, Uuid::parse_str(id).map_err(|_| StorageError::Corrupt)?)?
                    .ok_or(StorageError::Corrupt)
            })
            .collect()
    }
    pub fn get_selfcheck_attempt(&self, id: Uuid) -> StorageResult<Option<LearningAttempt>> {
        attempt(&*self.lock()?, id)
    }
    pub fn publish_advisory_grade(
        &self,
        run: &AnalysisRun,
        id: Uuid,
        feedback: &Feedback,
    ) -> StorageResult<()> {
        if run.attempt_id != Some(id)
            || run.purpose != "grading"
            || feedback.method != "ai_advisory"
            || feedback.grade == Grade::Pending
            || feedback.text.len() > 3000
            || sanitize(&feedback.text, "").map_err(|_| StorageError::DomainValidation)?
                != feedback.text
        {
            return Err(StorageError::DomainValidation);
        }
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        if !running(&tx, run)? || !current(&tx, run)? {
            return Err(StorageError::PolicyRejected);
        }
        let a = attempt(&tx, id)?.ok_or(StorageError::InvalidInput)?;
        let q = question(&tx, a.question_id)?.ok_or(StorageError::Corrupt)?;
        let valid_rubric = feedback.criteria.len() == q.question.criteria.len()
            && feedback
                .criteria
                .iter()
                .zip(&q.question.criteria)
                .all(|(grade, criterion)| {
                    grade.criterion == *criterion
                        && matches!(grade.outcome.as_str(), "pass" | "fail" | "uncertain")
                })
            && feedback
                .confidence
                .is_some_and(|c| c.is_finite() && (0.0..=1.0).contains(&c))
            && feedback.blocking_misconception.is_some();
        if !valid_rubric
            || (feedback.confidence.is_some_and(|c| c < 0.8)
                || feedback.criteria.iter().any(|g| g.outcome == "uncertain"))
                && feedback.grade != Grade::Uncertain
            || feedback.grade == Grade::Correct
                && (feedback.criteria.iter().any(|g| g.outcome != "pass")
                    || feedback.blocking_misconception == Some(true))
        {
            return Err(StorageError::DomainValidation);
        }
        if q.session_id != run.session_id
            || q.input_revision != run.input_revision
            || q.question.kind != "explain_why"
            || a.feedback.grade != Grade::Pending
        {
            return Err(StorageError::Constraint);
        }
        tx.execute(
            "UPDATE selfcheck_attempts SET grade=?2,feedback_json=?3,grading_run_id=?4 WHERE id=?1",
            params![
                id.to_string(),
                grade_text(&feedback.grade),
                encode(feedback)?,
                run.id.to_string()
            ],
        )
        .map_err(map_sqlite)?;
        tx.execute(
            "UPDATE analysis_runs SET status='published',finished_at=?2 WHERE id=?1",
            params![run.id.to_string(), self.now()?],
        )
        .map_err(map_sqlite)?;
        tx.commit().map_err(map_sqlite)
    }
}
fn grade_text(g: &Grade) -> &'static str {
    match g {
        Grade::Correct => "correct",
        Grade::Incorrect => "incorrect",
        Grade::Uncertain => "uncertain",
        Grade::Pending => "pending",
    }
}
fn question(c: &Connection, id: Uuid) -> StorageResult<Option<LearningQuestion>> {
    let row:Option<(String,String,String,bool,String,i64)>=c.query_row("SELECT q.document_id,q.concept_key,q.question_json,EXISTS(SELECT 1 FROM selfcheck_reveals r WHERE r.question_id=q.id),d.session_id,d.input_revision FROM selfcheck_questions q JOIN learning_documents d ON d.id=q.document_id WHERE q.id=?1 AND q.variant='initial'",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(map_sqlite)?;
    row.map(|(doc, key, text, revealed, session, revision)| {
        let question: Question = decode(&text)?;
        mochi_learning::parse_question(&question).map_err(|_| StorageError::Corrupt)?;
        Ok(LearningQuestion {
            id,
            document_id: Uuid::parse_str(&doc).map_err(|_| StorageError::Corrupt)?,
            concept_key: key,
            question,
            revealed,
            session_id: SessionId::parse(&session).map_err(|_| StorageError::Corrupt)?,
            input_revision: u64::try_from(revision).map_err(|_| StorageError::Corrupt)?,
        })
    })
    .transpose()
}
fn attempt(c: &Connection, id: Uuid) -> StorageResult<Option<LearningAttempt>> {
    let row:Option<(String,String,String,bool,String,String)>=c.query_row("SELECT question_id,answer,assistance,solution_seen,feedback_json,submitted_at FROM selfcheck_attempts WHERE id=?1",[id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional().map_err(map_sqlite)?;
    row.map(
        |(q, answer, assistance, solution_seen, json, submitted_at)| {
            Ok(LearningAttempt {
                id,
                question_id: Uuid::parse_str(&q).map_err(|_| StorageError::Corrupt)?,
                answer,
                assistance,
                solution_seen,
                feedback: decode(&json)?,
                submitted_at,
            })
        },
    )
    .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapturePolicy, CodingSessionRepository, ProjectRepository};
    use mochi_domain::{CodingSession, Project, UtcTimestamp};
    use mochi_learning::{Evidence, LearningAnalysisInput, MODEL, validate_generation};
    use std::sync::Arc;
    const NOW: &str = "2026-10-01T12:00:00Z";
    struct Clock;
    impl crate::Clock for Clock {
        fn now_rfc3339(&self) -> StorageResult<String> {
            Ok(NOW.into())
        }
    }
    fn fixture() -> (
        tempfile::TempDir,
        SqliteStore,
        LearningDocument,
        AnalysisRun,
    ) {
        let temp = tempfile::tempdir().expect("isolated data");
        let root = temp.path().canonicalize().expect("root");
        let store =
            SqliteStore::open(root.join("data/mochi.sqlite3"), Arc::new(Clock)).expect("store");
        let session = CodingSession::from_json(include_str!(
            "../../../packages/domain/fixtures/coding-session-cli-complete.json"
        ))
        .expect("domain fixture");
        let now = UtcTimestamp::parse(NOW).expect("date");
        let project = Project {
            id: session.data().project_id,
            display_name: "Synthetic".into(),
            root_path: root.to_string_lossy().into_owned(),
            repository_identity: None,
            created_at: now.clone(),
            last_seen_at: now,
        };
        store
            .create_project(
                &project,
                CapturePolicy {
                    tracking_enabled: false,
                    revision: 1,
                },
            )
            .expect("project");
        store.insert_session(&session).expect("session");
        store.lock().expect("connection").execute("INSERT INTO capture_episodes(id,project_id,source_key,source_json,state,revision,first_sequence,last_sequence,last_observed_at,finalized_at,end_reason,paused,restarted,late_evidence,fingerprint) VALUES(?1,?2,'synthetic','{}','finalized',2,1,1,?3,?3,'user_finalized',0,0,0,'synthetic')",params![session.data().id.to_string(),project.id.to_string(),NOW]).expect("fixture episode");
        let text="def total(values):\n    result = 0\n    for value in values:\n        result += value\n    return result".to_owned();
        let input = LearningAnalysisInput {
            schema_version: 1,
            session_id: session.data().id,
            input_revision: 2,
            policy_revision: 1,
            file_policy_fingerprint: hash("policy"),
            project_alias: "Synthetic".into(),
            coverage: "partial".into(),
            attribution_warning: "Uncertain authorship.".into(),
            stop_reason: "user_finalized".into(),
            language: "English".into(),
            evidence: vec![Evidence {
                id: "code:after:fixture".into(),
                kind: "code".into(),
                excerpt_hash: hash(&text),
                text,
                path: Some("calculator.py".into()),
                captured_at: NOW.into(),
                first_line: Some(1),
                truncated: false,
                verified_success: false,
            }],
            omissions: vec![],
            registry: vec!["algorithms.accumulation".into()],
        };
        let doc = validate_generation(
            include_bytes!("../../learning/fixtures/accumulation-generation.json"),
            input,
            &BTreeSet::new(),
            &BTreeSet::new(),
            MODEL,
        )
        .expect("validated document");
        let run = AnalysisRun {
            id: Uuid::new_v4(),
            session_id: session.data().id,
            input_revision: 2,
            policy_revision: 1,
            input_hash: doc.input_hash.clone(),
            request_hash: hash("synthetic-request"),
            model: MODEL.into(),
            purpose: "generation".into(),
            attempt_id: None,
        };
        (temp, store, doc, run)
    }
    #[test]
    fn durable_questions_idempotent_answers_reveal_and_cascade() {
        let (temp, store, doc, run) = fixture();
        store.begin_analysis(&run).expect("consent metadata");
        store.publish_learning(&run, &doc).expect("atomic publish");
        let q = doc.priorities[0].question_id;
        assert!(
            store
                .get_learning_question(doc.priorities[0].delayed_question_id)
                .expect("delayed hidden")
                .is_none()
        );
        let answer = "Zero is the additive identity. sk-syntheticFixture012345678901234567890";
        let id = Uuid::new_v4();
        let a = store
            .submit_selfcheck(id, q, answer, "independent")
            .expect("saved");
        assert_eq!(a.feedback.grade, Grade::Pending);
        assert!(!a.answer.contains("sk-synthetic"));
        assert_eq!(
            store
                .submit_selfcheck(id, q, answer, "independent")
                .expect("replay")
                .id,
            id
        );
        assert!(matches!(
            store.submit_selfcheck(id, q, "different answer", "independent"),
            Err(StorageError::Constraint)
        ));
        assert_eq!(store.selfcheck_attempts(q).expect("history").len(), 1);
        store.reveal_selfcheck(q).expect("reveal");
        let next = store
            .submit_selfcheck(
                Uuid::new_v4(),
                q,
                "I read the revealed explanation.",
                "independent",
            )
            .expect("practice");
        assert_eq!(next.assistance, "assisted");
        assert!(next.solution_seen);
        let path = store.path().to_path_buf();
        drop(store);
        let store = SqliteStore::open(path, Arc::new(Clock)).expect("reopen");
        assert_eq!(
            store
                .latest_learning(run.session_id)
                .expect("document")
                .expect("exists")
                .input_hash,
            doc.input_hash
        );
        assert_eq!(store.selfcheck_attempts(q).expect("answers").len(), 2);
        assert_eq!(store.learning_history().expect("exposure").0.len(), 1);
        store.delete_session(run.session_id).expect("cascade");
        for table in [
            "analysis_runs",
            "learning_documents",
            "selfcheck_questions",
            "selfcheck_attempts",
            "selfcheck_reveals",
            "learning_exposures",
        ] {
            let count: i64 = store
                .lock()
                .expect("connection")
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .expect("count");
            assert_eq!(count, 0, "{table}");
        }
        drop(temp);
    }
    #[test]
    fn revoked_policy_changed_revision_and_forged_runs_cannot_publish() {
        let (_temp, store, doc, run) = fixture();
        store.begin_analysis(&run).expect("authorized");
        let mut forged = run.clone();
        forged.request_hash = hash("another request");
        assert_eq!(
            store.publish_learning(&forged, &doc),
            Err(StorageError::PolicyRejected)
        );
        assert!(
            store
                .latest_learning(run.session_id)
                .expect("none")
                .is_none()
        );

        store
            .lock()
            .expect("connection")
            .execute(
                "UPDATE capture_episodes SET revision=3 WHERE id=?1",
                [run.session_id.to_string()],
            )
            .expect("late evidence");
        assert_eq!(
            store.publish_learning(&run, &doc),
            Err(StorageError::PolicyRejected)
        );
        store.recover_analysis().expect("restart cancellation");
        let state: String = store
            .lock()
            .expect("connection")
            .query_row(
                "SELECT status FROM analysis_runs WHERE id=?1",
                [run.id.to_string()],
                |r| r.get(0),
            )
            .expect("status");
        assert_eq!(state, "cancelled");
    }
    #[test]
    fn advisory_grade_is_bound_to_the_immutable_attempt() {
        let (_temp, store, doc, run) = fixture();
        store.begin_analysis(&run).expect("run");
        store.publish_learning(&run, &doc).expect("publish");
        let id = Uuid::new_v4();
        store
            .submit_selfcheck(
                id,
                doc.priorities[0].question_id,
                "Zero does not change the sum.",
                "independent",
            )
            .expect("answer");
        let grading = AnalysisRun {
            id: Uuid::new_v4(),
            purpose: "grading".into(),
            request_hash: hash("grade"),
            attempt_id: Some(id),
            ..run
        };
        store.begin_analysis(&grading).expect("grade consent");
        let feedback = Feedback {
            criteria: doc.priorities[0]
                .concept
                .question
                .criteria
                .iter()
                .enumerate()
                .map(|(i, s)| mochi_learning::CriterionGrade {
                    criterion: s.clone(),
                    outcome: if i == 0 { "pass" } else { "uncertain" }.into(),
                })
                .collect(),
            blocking_misconception: Some(false),
            grade: Grade::Uncertain,
            method: "ai_advisory".into(),
            confidence: Some(0.7),
            text: "The second criterion remains uncertain.".into(),
        };
        assert_eq!(
            store.publish_advisory_grade(&grading, Uuid::new_v4(), &feedback),
            Err(StorageError::DomainValidation)
        );
        store
            .publish_advisory_grade(&grading, id, &feedback)
            .expect("advisory save");
        assert_eq!(
            store
                .get_selfcheck_attempt(id)
                .expect("load")
                .expect("exists")
                .feedback
                .grade,
            Grade::Uncertain
        );
        assert!(
            store
                .publish_advisory_grade(&grading, id, &feedback)
                .is_err()
        );
    }
}
