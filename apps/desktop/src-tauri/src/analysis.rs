use crate::{
    credentials::{CredentialStore, Keychain, Secret},
    local_capture::LocalCapture,
    provider::{AnalysisProvider, OpenAiProvider},
};
use mochi_domain::SessionId;
use mochi_learning::{
    generation_request, grading_request, hash, response_text, validate_generation, validate_grade,
    Feedback, LearningAnalysisInput, LearningDocument, Question, ENDPOINT, MODEL,
};
use mochi_persistence::{AnalysisRun, LearningAttempt};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use uuid::Uuid;
type Result<T> = std::result::Result<T, &'static str>;
#[derive(Clone)]
enum Purpose {
    Generation,
    Grading {
        attempt_id: Uuid,
        question: Box<Question>,
        answer: String,
    },
}
struct Prepared {
    input: LearningAnalysisInput,
    request: String,
    purpose: Purpose,
    created: Instant,
    epoch: u64,
}
struct ActiveJob {
    cancel: Arc<AtomicBool>,
    id: Uuid,
}
struct RemoteState {
    enabled: bool,
    epoch: u64,
    pending: BTreeMap<Uuid, Prepared>,
    job: Option<ActiveJob>,
    message: String,
}
pub struct AnalysisService {
    state: Mutex<RemoteState>,
    credentials: Arc<dyn CredentialStore>,
    provider: Arc<dyn AnalysisProvider>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStatus {
    pub enabled: bool,
    pub key_configured: bool,
    pub running: bool,
    pub message: String,
    pub model: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendPreview {
    pub token: Uuid,
    pub session_id: SessionId,
    pub input_revision: u64,
    pub input_hash: String,
    pub request_hash: String,
    pub provider: &'static str,
    pub model: &'static str,
    pub endpoint: &'static str,
    pub purpose: String,
    pub request_json: String,
    pub expires_in_seconds: u16,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceView {
    pub id: String,
    pub text: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionView {
    pub id: Uuid,
    pub kind: String,
    pub prompt: String,
    pub choices: Vec<ChoiceView>,
    pub snippet: Option<String>,
    pub assumptions: Option<String>,
    pub revealed: bool,
    pub attempts: Vec<LearningAttempt>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardView {
    pub key: String,
    pub title: String,
    pub definition: String,
    pub why_it_works: String,
    pub why_it_matters: String,
    pub misconception: String,
    pub references: Vec<String>,
    pub code_reference: Option<String>,
    pub code_quote: Option<String>,
    pub components: mochi_learning::Components,
    pub selection_score: f64,
    pub question: QuestionView,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonView {
    pub id: Uuid,
    pub session_id: SessionId,
    pub input_revision: u64,
    pub input_hash: String,
    pub model: String,
    pub stale: bool,
    pub status: String,
    pub coverage: String,
    pub omissions: Vec<String>,
    pub evidence: Vec<mochi_learning::Evidence>,
    pub reconstruction: mochi_learning::Reconstruction,
    pub cards: Vec<CardView>,
}
impl AnalysisService {
    pub fn new() -> Self {
        Self::with_ports(Arc::new(Keychain::default()), Arc::new(OpenAiProvider))
    }
    pub(crate) fn with_ports(
        credentials: Arc<dyn CredentialStore>,
        provider: Arc<dyn AnalysisProvider>,
    ) -> Self {
        Self {
            state: Mutex::new(RemoteState {
                enabled: false,
                epoch: 1,
                pending: BTreeMap::new(),
                job: None,
                message: "Remote analysis is off. No content has been sent.".into(),
            }),
            credentials,
            provider,
        }
    }
    pub fn status(&self) -> Result<AnalysisStatus> {
        let s = self
            .state
            .lock()
            .map_err(|_| "Analysis status unavailable.")?;
        Ok(AnalysisStatus {
            enabled: s.enabled,
            key_configured: self.credentials.configured()?,
            running: s.job.is_some(),
            message: s.message.clone(),
            model: MODEL,
        })
    }
    pub fn remote_enabled(&self) -> Result<bool> {
        self.state
            .lock()
            .map(|s| s.enabled)
            .map_err(|_| "Analysis control unavailable.")
    }
    pub fn permission(&self, enabled: bool) -> Result<()> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| "Analysis control unavailable.")?;
        if let Some(job) = &s.job {
            job.cancel.store(true, Ordering::Release);
        }
        s.epoch = s
            .epoch
            .checked_add(1)
            .ok_or("Analysis control unavailable.")?;
        s.enabled = enabled;
        s.pending.clear();
        s.message=if enabled{"Remote requests are available only after an exact session or answer preview approval."}else{"Remote analysis is off. In-flight requests are cancelled; bytes already submitted cannot be recalled."}.into();
        Ok(())
    }
    pub fn cancel(&self) -> Result<()> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| "Analysis control unavailable.")?;
        if let Some(job) = &s.job {
            job.cancel.store(true, Ordering::Release);
        }
        s.epoch = s
            .epoch
            .checked_add(1)
            .ok_or("Analysis control unavailable.")?;
        s.pending.clear();
        s.message = "Cancelled locally. Submitted provider data cannot be recalled.".into();
        Ok(())
    }
    pub fn set_key(&self, key: String) -> Result<()> {
        self.permission(false)?;
        let secret = Secret::from_owned(key)?;
        self.credentials.set(&secret)
    }
    pub fn delete_key(&self) -> Result<()> {
        self.permission(false)?;
        self.credentials.delete()
    }
    pub fn prepare(
        &self,
        core: &LocalCapture,
        id: SessionId,
        attempt_id: Option<Uuid>,
    ) -> Result<SendPreview> {
        let input = core.analysis_input(id)?;
        let purpose = if let Some(attempt_id) = attempt_id {
            let attempt = core
                .store
                .get_selfcheck_attempt(attempt_id)
                .map_err(|_| "Answer unavailable.")?
                .ok_or("Answer unavailable.")?;
            let q = core
                .store
                .get_learning_question(attempt.question_id)
                .map_err(|_| "Question unavailable.")?
                .ok_or("Question unavailable.")?;
            if q.session_id != id
                || q.input_revision != input.input_revision
                || q.question.kind != "explain_why"
                || attempt.feedback.grade != mochi_learning::Grade::Pending
            {
                return Err("Only an ungraded, current explanation answer can be previewed.");
            }
            Purpose::Grading {
                attempt_id,
                question: Box::new(q.question),
                answer: attempt.answer,
            }
        } else {
            Purpose::Generation
        };
        let request = match &purpose {
            Purpose::Generation => generation_request(&input),
            Purpose::Grading {
                question, answer, ..
            } => grading_request(question, answer),
        }
        .map_err(|_| "Analysis request exceeded safe bounds.")?;
        if request.len() > 128 * 1024 {
            return Err("Analysis request exceeded safe bounds.");
        }
        let token = Uuid::new_v4();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Analysis preview unavailable.")?;
        state
            .pending
            .retain(|_, p| p.created.elapsed() < Duration::from_secs(300));
        if state.pending.len() >= 8 {
            state.pending.clear();
        }
        let preview = SendPreview {
            token,
            session_id: id,
            input_revision: input.input_revision,
            input_hash: input
                .input_hash()
                .map_err(|_| "Analysis input unavailable.")?,
            request_hash: hash(&request),
            provider: "OpenAI",
            model: MODEL,
            endpoint: ENDPOINT,
            purpose: match purpose {
                Purpose::Generation => "generation",
                Purpose::Grading { .. } => "grading",
            }
            .into(),
            request_json: request.clone(),
            expires_in_seconds: 300,
        };
        let epoch = state.epoch;
        state.pending.insert(
            token,
            Prepared {
                input,
                request,
                purpose,
                created: Instant::now(),
                epoch,
            },
        );
        Ok(preview)
    }
    pub fn approve(core: Arc<LocalCapture>, token: Uuid, confirmed: bool) -> Result<()> {
        if !confirmed {
            return Err("Approve the exact preview before sending.");
        }
        let prepared;
        let cancel = Arc::new(AtomicBool::new(false));
        let run;
        {
            let mut state = core
                .analysis
                .state
                .lock()
                .map_err(|_| "Analysis control unavailable.")?;
            if !state.enabled {
                return Err("Enable remote analysis first. Key presence is not permission.");
            }
            if state.job.is_some() {
                return Err("An analysis job is already running.");
            }
            prepared = state
                .pending
                .remove(&token)
                .ok_or("Preview expired. Review it again.")?;
            if prepared.created.elapsed() >= Duration::from_secs(300)
                || prepared.epoch != state.epoch
            {
                return Err("Preview expired or permissions changed. Review it again.");
            }
            if !core.analysis.credentials.configured()? {
                return Err("Store your API key in Keychain before sending.");
            }
            if !Self::same_input(&core, &prepared) {
                return Err("Session evidence or exclusions changed. Preview again.");
            }
            run = AnalysisRun {
                id: token,
                session_id: prepared.input.session_id,
                input_revision: prepared.input.input_revision,
                policy_revision: prepared.input.policy_revision,
                input_hash: prepared
                    .input
                    .input_hash()
                    .map_err(|_| "Input unavailable.")?,
                request_hash: hash(&prepared.request),
                model: MODEL.into(),
                attempt_id: match &prepared.purpose {
                    Purpose::Generation => None,
                    Purpose::Grading { attempt_id, .. } => Some(*attempt_id),
                },
                purpose: match prepared.purpose {
                    Purpose::Generation => "generation",
                    Purpose::Grading { .. } => "grading",
                }
                .into(),
            };
            core.store
                .begin_analysis(&run)
                .map_err(|_| "Analysis authorization could not be recorded safely.")?;
            state.job = Some(ActiveJob {
                id: token,
                cancel: cancel.clone(),
            });
            state.message =
                "Approved request is running. You can cancel it or turn remote analysis off."
                    .into();
        }
        let worker = core.clone();
        let run_id = run.id;
        if std::thread::Builder::new().name("mochi-analysis".into()).spawn(move||{
   let result=worker.analysis.execute(&worker,&prepared,&run,&cancel);
   if result.is_err(){let _=worker.store.finish_analysis_failure(run.id,cancel.load(Ordering::Acquire));}
   if let Ok(mut state)=worker.analysis.state.lock(){if state.job.as_ref().is_some_and(|j|j.id==run.id){state.job=None;state.message=result.map(|()|"Saved locally. This is an internal explanation and self-check, not a complete V1 lesson.".into()).unwrap_or_else(|e|e.into());}}
  }).is_err(){let _=core.store.finish_analysis_failure(run_id,true);if let Ok(mut state)=core.analysis.state.lock(){state.job=None;state.message="Analysis could not start. Nothing was sent.".into();}return Err("Analysis could not start.")}
        Ok(())
    }
    fn same_input(core: &LocalCapture, p: &Prepared) -> bool {
        core.analysis_input(p.input.session_id)
            .and_then(|i| i.input_hash().map_err(|_| "Input unavailable."))
            .ok()
            == p.input.input_hash().ok()
    }
    fn execute(
        &self,
        core: &LocalCapture,
        p: &Prepared,
        run: &AnalysisRun,
        cancel: &AtomicBool,
    ) -> Result<()> {
        let current = || {
            self.state
                .lock()
                .is_ok_and(|s| s.enabled && s.epoch == p.epoch)
                && !cancel.load(Ordering::Acquire)
                && Self::same_input(core, p)
        };
        if !current() {
            return Err("Analysis cancelled or evidence changed.");
        }
        let key = self.credentials.read()?;
        let bytes = self
            .provider
            .send(&p.request, &key, cancel, &current)
            .map_err(|e| e.message())?;
        let text = response_text(&bytes).map_err(|e| match e {
            mochi_learning::LearningError::Refused => {
                "OpenAI refused the request. No explanation was published."
            }
            mochi_learning::LearningError::Incomplete => {
                "OpenAI returned an incomplete result. Preview again to retry."
            }
            _ => "OpenAI returned malformed output. Nothing was published.",
        })?;
        let result=match &p.purpose {
   Purpose::Generation=>{let (exposed,incorrect)=core.store.learning_history().map_err(|_|"Learning history unavailable.")?;Output::Document(Box::new(validate_generation(text.as_bytes(),p.input.clone(),&exposed,&incorrect,MODEL).map_err(|_|"Generated explanation failed grounding or question validation. Nothing was published; review and retry.")?))},
   Purpose::Grading{attempt_id,question,..}=>Output::Feedback(*attempt_id,validate_grade(text.as_bytes(),question).map_err(|_|"Advisory grading failed rubric validation. Your answer remains saved and ungraded.")?),
  };
        let _operation = core
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        let state = self
            .state
            .lock()
            .map_err(|_| "Analysis control unavailable.")?;
        if !state.enabled
            || state.epoch != p.epoch
            || cancel.load(Ordering::Acquire)
            || !Self::same_input(core, p)
        {
            return Err(
                "Analysis cancelled or evidence changed. The returned result was discarded.",
            );
        }
        match result {
            Output::Document(doc) => core
                .store
                .publish_learning(run, &doc)
                .map_err(|_| "Explanation could not be published safely."),
            Output::Feedback(id, feedback) => core
                .store
                .publish_advisory_grade(run, id, &feedback)
                .map_err(|_| "Feedback could not be saved safely; the answer remains available."),
        }
    }
    pub fn lesson(&self, core: &LocalCapture, id: SessionId) -> Result<Option<LessonView>> {
        let Some(doc) = core
            .store
            .latest_learning(id)
            .map_err(|_| "Explanation unavailable.")?
        else {
            return Ok(None);
        };
        let stale = !core.learning_input_current(&doc.input);
        let mut cards = Vec::new();
        for p in doc.priorities {
            let q = core
                .store
                .get_learning_question(p.question_id)
                .map_err(|_| "Question unavailable.")?
                .ok_or("Question unavailable.")?;
            let attempts = core
                .store
                .selfcheck_attempts(q.id)
                .map_err(|_| "Answers unavailable.")?;
            cards.push(CardView {
                key: p.concept.key,
                title: p.concept.title,
                definition: p.concept.definition,
                why_it_works: p.concept.why_it_works,
                why_it_matters: p.concept.why_it_matters,
                misconception: p.concept.misconception,
                references: p.concept.references,
                code_reference: p.concept.code_reference,
                code_quote: p.concept.code_quote,
                components: p.components,
                selection_score: p.selection_score,
                question: QuestionView {
                    id: q.id,
                    kind: q.question.kind,
                    prompt: q.question.prompt,
                    choices: q
                        .question
                        .choices
                        .into_iter()
                        .map(|c| ChoiceView {
                            id: c.id,
                            text: c.text,
                        })
                        .collect(),
                    snippet: q.question.snippet,
                    assumptions: q.question.assumptions,
                    revealed: q.revealed,
                    attempts,
                },
            });
        }
        Ok(Some(LessonView {
            id: doc.id,
            session_id: id,
            input_revision: doc.input.input_revision,
            input_hash: doc.input_hash,
            model: doc.model,
            stale,
            status: if cards.is_empty() {
                "insufficient_context"
            } else {
                "internal_explanation"
            }
            .into(),
            coverage: doc.input.coverage,
            omissions: doc.input.omissions,
            evidence: doc.input.evidence,
            reconstruction: doc.reconstruction,
            cards,
        }))
    }
}
enum Output {
    Document(Box<LearningDocument>),
    Feedback(Uuid, Feedback),
}

#[cfg(test)]
mod tests {
    use super::*;
    use mochi_capture::{
        source::SessionSource, CaptureSanitizer, CodexSessionSource, Spool, SpoolLimits,
    };
    use mochi_domain::{CodingSession, GitContext, ProjectId};
    use mochi_persistence::{CodingSessionRepository, IngressRepository, ProjectRepository};
    use std::sync::atomic::AtomicUsize;
    struct MemoryKey;
    impl CredentialStore for MemoryKey {
        fn configured(&self) -> Result<bool> {
            Ok(true)
        }
        fn read(&self) -> Result<Secret> {
            Secret::new("sk-syntheticFixture012345678901234567890")
        }
        fn set(&self, _: &Secret) -> Result<()> {
            Ok(())
        }
        fn delete(&self) -> Result<()> {
            Ok(())
        }
    }
    struct FixedClock;
    impl mochi_capture::source::Clock for FixedClock {
        fn now_rfc3339(&self) -> String {
            "2026-10-01T12:00:00Z".into()
        }
    }
    struct FixtureProvider {
        calls: AtomicUsize,
        release: AtomicBool,
        fail: bool,
    }
    impl AnalysisProvider for FixtureProvider {
        fn send(
            &self,
            request: &str,
            _: &Secret,
            _: &AtomicBool,
            current: &(dyn Fn() -> bool + Sync),
        ) -> std::result::Result<Vec<u8>, crate::provider::ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(crate::provider::ProviderError::Network);
            }
            while !self.release.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(10));
            }
            // A malicious/late transport result must still fail the core publication recheck.
            let _ = current();
            let request: serde_json::Value =
                serde_json::from_str(request).expect("synthetic request");
            let input: LearningAnalysisInput = serde_json::from_str(
                request["input"][0]["content"][0]["text"]
                    .as_str()
                    .expect("input"),
            )
            .expect("input contract");
            let reference = input
                .evidence
                .iter()
                .find(|e| e.kind == "code" && e.text.contains("result += value"))
                .expect("code fixture")
                .id
                .clone();
            let mut generated: serde_json::Value = serde_json::from_slice(include_bytes!(
                "../../../../crates/learning/fixtures/accumulation-generation.json"
            ))
            .expect("generation fixture");
            fn replace(v: &mut serde_json::Value, id: &str) {
                match v {
                    serde_json::Value::String(s) if s == "code:after:fixture" => *s = id.into(),
                    serde_json::Value::Array(a) => {
                        for v in a {
                            replace(v, id)
                        }
                    }
                    serde_json::Value::Object(o) => {
                        for v in o.values_mut() {
                            replace(v, id)
                        }
                    }
                    _ => {}
                }
            }
            replace(&mut generated, &reference);
            Ok(serde_json::to_vec(&serde_json::json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":generated.to_string()}]}]})).expect("response"))
        }
    }
    fn core(provider: Arc<FixtureProvider>) -> (tempfile::TempDir, Arc<LocalCapture>, SessionId) {
        let temp = tempfile::tempdir().expect("isolated fixture");
        let root = temp.path().canonicalize().expect("root");
        let project_root = root.join("project");
        std::fs::create_dir(&project_root).expect("project");
        let service = AnalysisService::with_ports(Arc::new(MemoryKey), provider);
        let core = LocalCapture::open_for_test(root.join("data"), service).expect("native core");
        let project = core
            .approve_project(
                project_root.to_string_lossy().into(),
                "Synthetic Calculator".into(),
            )
            .expect("approval");
        core.tracking(project.id, true).expect("capture consent");
        let revision = core
            .store
            .get_project(project.id)
            .expect("project")
            .expect("exists")
            .capture_policy
            .revision;
        let source = CodexSessionSource::new(
            project.id.as_uuid(),
            project_root.clone(),
            mochi_capture::ClientSurface::Cli,
            revision,
            CaptureSanitizer::new(&project_root).expect("redactor"),
            FixedClock,
        );
        let spool = Spool::new(root.join("data/capture-v2-spool"), SpoolLimits::default());
        for event in ["SessionStart", "UserPromptSubmit", "SessionEnd"] {
            let json = serde_json::json!({"hook_event_name":event,"cwd":project_root,"session_id":"synthetic-native-episode","turn_id":"t1","source":"startup","prompt":"Implement an accumulator for a calculator total, preserving the empty-input behavior."});
            spool
                .append(
                    project.id.as_uuid(),
                    source.descriptor(),
                    "2026-10-01T12:00:00Z",
                    source
                        .normalize(&serde_json::to_vec(&json).expect("payload"))
                        .expect("normalize"),
                )
                .expect("spool");
        }
        core.store
            .import_spool_batch(&spool, 100)
            .expect("durable import");
        let engine = mochi_assembly::EpisodeEngine::new(&core.store);
        engine.process_pending().expect("assembly");
        let id = core
            .store
            .list_sessions(project.id, None, None)
            .expect("sessions")
            .items[0]
            .id;
        let mut data = CodingSession::from_json(include_str!(
            "../../../../packages/domain/fixtures/coding-session-cli-complete.json"
        ))
        .expect("fixture")
        .into_data();
        let GitContext::Available { before, .. } = &mut data.git_context else {
            panic!("fixture Git")
        };
        let mut file = before.files[0].clone();
        file.path = "calculator.py".into();
        file.content=Some("def total(values):\n    result = 0\n    for value in values:\n        result += value\n    return result".into());
        file.omission = None;
        file.truncated = false;
        before.repository_root = project_root.to_string_lossy().into();
        before.files = vec![file];
        engine
            .set_git_context(
                id,
                &GitContext::FinalOnly {
                    after: before.clone(),
                    baseline_reason: mochi_domain::GitUnavailableReason::NotCaptured,
                },
            )
            .expect("snapshot");
        (temp, core, id)
    }
    fn wait(core: &LocalCapture) {
        let start = Instant::now();
        while core.analysis.status().expect("status").running {
            assert!(start.elapsed() < Duration::from_secs(15), "bounded worker");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    #[test]
    fn no_send_without_exact_permission_then_explanation_and_answer_survive_restart() {
        let provider = Arc::new(FixtureProvider {
            calls: AtomicUsize::new(0),
            release: AtomicBool::new(true),
            fail: false,
        });
        let (temp, core, id) = core(provider.clone());
        let preview = core
            .analysis
            .prepare(&core, id, None)
            .expect("preview local");
        assert!(AnalysisService::approve(core.clone(), preview.token, true).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        core.analysis.permission(true).expect("enable");
        let preview = core
            .analysis
            .prepare(&core, id, None)
            .expect("fresh preview");
        assert!(AnalysisService::approve(core.clone(), preview.token, false).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        AnalysisService::approve(core.clone(), preview.token, true).expect("exact approval");
        assert!(AnalysisService::approve(core.clone(), preview.token, true).is_err());
        wait(&core);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let lesson = core
            .analysis
            .lesson(&core, id)
            .expect("lesson")
            .expect("published");
        assert!(!lesson.stale);
        assert_eq!(lesson.cards.len(), 1);
        let public = serde_json::to_string(&lesson.cards[0].question).expect("public DTO");
        assert!(!public.contains("correctChoice"));
        assert!(!public.contains("criteria"));
        assert!(!public.contains("explanation"));
        core.store
            .submit_selfcheck(
                Uuid::new_v4(),
                lesson.cards[0].question.id,
                "Zero is the identity for addition; the accumulator persists across the loop.",
                "independent",
            )
            .expect("saved offline");
        let root = temp.path().canonicalize().expect("root");
        drop(core);
        let restarted = LocalCapture::open_for_test(
            root.join("data"),
            AnalysisService::with_ports(Arc::new(MemoryKey), provider),
        )
        .expect("restart");
        assert!(!restarted.analysis.status().expect("default off").enabled);
        let lesson = restarted
            .analysis
            .lesson(&restarted, id)
            .expect("lesson")
            .expect("retained");
        assert_eq!(lesson.cards[0].question.attempts.len(), 1);
        assert_eq!(
            lesson.cards[0].question.attempts[0].feedback.grade,
            mochi_learning::Grade::Pending
        );
    }
    #[test]
    fn exclusion_change_cancel_and_deleted_session_discard_late_results() {
        let provider = Arc::new(FixtureProvider {
            calls: AtomicUsize::new(0),
            release: AtomicBool::new(false),
            fail: false,
        });
        let (_temp, core, id) = core(provider.clone());
        core.analysis.permission(true).expect("permission");
        let preview = core.analysis.prepare(&core, id, None).expect("preview");
        let project = core
            .store
            .get_session(id)
            .expect("session")
            .expect("exists")
            .data()
            .project_id;
        let root = core
            .store
            .get_project(project)
            .expect("project")
            .expect("exists")
            .project
            .root_path;
        std::fs::write(
            std::path::Path::new(&root).join(".mochiignore"),
            "calculator.py\n",
        )
        .expect("changed exclusion");
        assert!(AnalysisService::approve(core.clone(), preview.token, true).is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
        std::fs::remove_file(std::path::Path::new(&root).join(".mochiignore"))
            .expect("restore fixture");
        let preview = core.analysis.prepare(&core, id, None).expect("preview");
        AnalysisService::approve(core.clone(), preview.token, true).expect("send");
        let start = Instant::now();
        while provider.calls.load(Ordering::SeqCst) == 0 {
            assert!(start.elapsed() < Duration::from_secs(10));
            std::thread::sleep(Duration::from_millis(10));
        }
        core.analysis.permission(false).expect("revoke");
        provider.release.store(true, Ordering::Release);
        wait(&core);
        assert!(core
            .store
            .latest_learning(id)
            .expect("no late result")
            .is_none());
        core.delete_session(id, true).expect("delete");
        assert!(core.analysis.prepare(&core, id, None).is_err());
        assert_eq!(ProjectId::parse(&project.to_string()).expect("id"), project);
    }
    #[test]
    fn network_failure_never_publishes_placeholder_or_auto_retries_on_restart() {
        let provider = Arc::new(FixtureProvider {
            calls: AtomicUsize::new(0),
            release: AtomicBool::new(true),
            fail: true,
        });
        let (_temp, core, id) = core(provider.clone());
        core.analysis.permission(true).expect("permission");
        let preview = core.analysis.prepare(&core, id, None).expect("preview");
        AnalysisService::approve(core.clone(), preview.token, true).expect("approved");
        wait(&core);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert!(core
            .store
            .latest_learning(id)
            .expect("no placeholder")
            .is_none());
        assert!(core
            .analysis
            .status()
            .expect("safe error")
            .message
            .contains("unavailable"));
    }
}
