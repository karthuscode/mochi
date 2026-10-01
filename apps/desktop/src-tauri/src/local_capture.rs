use fs2::FileExt;
use mochi_assembly::EpisodeEngine;
use mochi_bridge::CaptureBridge;
use mochi_capture::{source::Clock, GitCliContextReader, SystemClock};
use mochi_domain::{Project, ProjectId, SessionId, UtcTimestamp};
use mochi_integration::{
    CodexDetector, CodexDetectorOptions, CodexInstaller, Compatibility, InstallPlan,
    InstallPreview, InstallRequest, IntegrationDetectionService,
};
use mochi_persistence::{
    CapturePolicy, CodingSessionRepository, ProjectRepository, SessionCursor, SqliteStore,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

type Result<T> = std::result::Result<T, &'static str>;
struct PendingPlan {
    project_id: ProjectId,
    plan: InstallPlan,
    created: Instant,
}
pub struct LocalCapture {
    pub store: SqliteStore,
    bridge: CaptureBridge,
    installer: CodexInstaller,
    plans: Mutex<BTreeMap<Uuid, PendingPlan>>,
    pub(crate) operations: Mutex<()>,
    pub(crate) analysis: crate::analysis::AnalysisService,
    helper: PathBuf,
    policy_root: PathBuf,
    spool_root: PathBuf,
    _profile_lock: File,
    status: Mutex<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    pub id: ProjectId,
    pub name: String,
    pub root: String,
    pub tracking: bool,
    pub policy_revision: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: SessionId,
    pub started_at: UtcTimestamp,
    pub ended_at: Option<UtcTimestamp>,
    pub capture_state: String,
    pub coverage: String,
    pub revision: u64,
    pub paused: bool,
    pub restarted: bool,
    pub late_evidence: bool,
    pub continuation_of: Option<SessionId>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionList {
    pub items: Vec<SessionView>,
    pub next: Option<ListCursor>,
}
#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListCursor {
    pub started_at: UtcTimestamp,
    pub session_id: SessionId,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub id: String,
    pub sequence: u64,
    pub title: String,
    pub text: String,
    pub truncated: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeView {
    pub path: String,
    pub content: Option<String>,
    pub omitted: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    pub session: SessionView,
    pub events: Vec<EventView>,
    pub next_sequence: Option<u64>,
    pub event_count: usize,
    pub code: Vec<CodeView>,
    pub git_notice: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalStatus {
    pub spool_eviction_count: u64,
    pub schema_version: u8,
    pub message: String,
    pub remote_analysis_enabled: bool,
}

impl LocalCapture {
    pub fn open(data: PathBuf, helper: PathBuf) -> Result<Arc<Self>> {
        Self::open_with_service(data, helper, crate::analysis::AnalysisService::new(), true)
    }
    fn open_with_service(
        data: PathBuf,
        helper: PathBuf,
        analysis: crate::analysis::AnalysisService,
        start_worker: bool,
    ) -> Result<Arc<Self>> {
        mochi_capture::authorization::AuthorizationStore::create(data.clone())
            .map_err(|_| "Local storage permissions unavailable.")?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        let lock = options
            .open(data.join("core.lock"))
            .map_err(|_| "Local profile unavailable.")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let metadata = lock.metadata().map_err(|_| "Local profile unavailable.")?;
            if !metadata.is_file()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.nlink() != 1
            {
                return Err("Local profile permissions unavailable.");
            }
            lock.set_permissions(std::fs::Permissions::from_mode(0o600))
                .map_err(|_| "Local profile permissions unavailable.")?;
        }
        FileExt::try_lock_exclusive(&lock)
            .map_err(|_| "Mochi is already processing this local profile.")?;
        let store = SqliteStore::open_system(data.join("mochi.sqlite3"))
            .map_err(|_| "Local storage could not be opened safely.")?;
        let policy_root = data.join("capture-policy");
        let spool_root = data.join("capture-v2-spool");
        let bridge = CaptureBridge::new(policy_root.clone(), spool_root.clone())
            .map_err(|_| "Capture policy unavailable.")?;
        let installer = CodexInstaller::new(data.join("integration"))
            .map_err(|_| "Integration recovery unavailable.")?;
        let core = Arc::new(Self {
            store,
            bridge,
            installer,
            plans: Mutex::new(BTreeMap::new()),
            operations: Mutex::new(()),
            analysis,
            helper,
            policy_root,
            spool_root,
            _profile_lock: lock,
            status: Mutex::new(
                "Local capture is off until a project is approved and connected.".into(),
            ),
        });
        let mut after = None;
        loop {
            let projects = core
                .store
                .list_projects(after, 100)
                .map_err(|_| "Projects unavailable.")?;
            if projects.is_empty() {
                break;
            }
            for p in &projects {
                core.bridge
                    .reconcile(&core.store, p.project.id)
                    .map_err(|_| "Capture policy reconciliation failed safely.")?;
            }
            after = projects.last().map(|p| p.project.id);
            if projects.len() < 100 {
                break;
            }
        }
        core.store
            .mark_capture_restart()
            .map_err(|_| "Capture recovery unavailable.")?;
        core.store
            .recover_analysis()
            .map_err(|_| "Analysis recovery unavailable.")?;
        if !start_worker {
            return Ok(core);
        }
        let weak = Arc::downgrade(&core);
        std::thread::Builder::new()
            .name("mochi-local-capture".into())
            .spawn(move || loop {
                let Some(core) = weak.upgrade() else { break };
                if let Ok(_guard) = core.operations.try_lock() {
                    let message = match core.tick() {
                        Ok(()) => "Local evidence processing is running.",
                        Err(e) => e,
                    };
                    if let Ok(mut status) = core.status.lock() {
                        *status = message.into();
                    }
                }
                drop(core);
                std::thread::sleep(Duration::from_secs(1));
            })
            .map_err(|_| "Local processing could not start.")?;
        Ok(core)
    }
    #[cfg(test)]
    pub(crate) fn open_for_test(
        data: PathBuf,
        analysis: crate::analysis::AnalysisService,
    ) -> Result<Arc<Self>> {
        Self::open_with_service(data, PathBuf::from("/synthetic/helper"), analysis, false)
    }
    fn tick(&self) -> Result<()> {
        self.bridge
            .import(&self.store)
            .map_err(|_| "Evidence import paused. Retained events will be retried.")?;
        let engine = EpisodeEngine::new(&self.store);
        let outcome = engine
            .process_pending()
            .map_err(|_| "Episode processing paused safely.")?;
        if outcome.deferred > 0 {
            return Err("Some evidence could not be assembled within safe bounds. Pause capture and inspect the session.");
        }
        // One bounded Git read per tick; ready histories are never decoded in a poll.
        if let Some(id) = self
            .store
            .next_git_episode()
            .map_err(|_| "Git queue unavailable.")?
        {
            self.bridge
                .collect_session_git(&self.store, id, &GitCliContextReader, |context| {
                    engine
                        .set_git_context(id, context)
                        .map_err(|_| mochi_bridge::BridgeError::Storage)
                })
                .map_err(|_| {
                    "Git context was not collected; check capture consent and permissions."
                })?;
        }

        Ok(())
    }
    pub(crate) fn analysis_input(
        &self,
        id: SessionId,
    ) -> Result<mochi_learning::LearningAnalysisInput> {
        let session = self
            .store
            .finalized_session(id)
            .map_err(|_| "Session unavailable.")?
            .ok_or("Finish the observed session before preparing analysis.")?;
        let episode = self
            .store
            .get_episode(id)
            .map_err(|_| "Session metadata unavailable.")?
            .ok_or("Session metadata unavailable.")?;
        let project = self
            .store
            .get_project(session.data().project_id)
            .map_err(|_| "Project unavailable.")?
            .filter(|p| p.deleted_at.is_none())
            .ok_or("Project unavailable.")?;
        let policy =
            mochi_privacy::FilePolicy::load(std::path::Path::new(&project.project.root_path))
                .map_err(|_| "Current exclusions could not be checked. No analysis is prepared.")?;
        mochi_learning::prepare_input(&session,&project.project,episode.revision,project.capture_policy.revision,episode.end_reason.as_deref().unwrap_or("unknown"),&policy).map_err(|e| match e {mochi_learning::LearningError::InsufficientContext=>"Insufficient technical evidence for a useful explanation. A prompt alone is not enough.",_=>"Analysis content could not be prepared within safe privacy bounds."})
    }
    pub(crate) fn learning_input_current(
        &self,
        input: &mochi_learning::LearningAnalysisInput,
    ) -> bool {
        let Ok(Some(ep)) = self.store.get_episode(input.session_id) else {
            return false;
        };
        if ep.state != mochi_persistence::EpisodeState::Finalized
            || ep.revision != input.input_revision
        {
            return false;
        }
        let Ok(Some(project)) = self.store.get_project(ep.source.project_id) else {
            return false;
        };
        if project.deleted_at.is_some() || project.capture_policy.revision != input.policy_revision
        {
            return false;
        }
        let Ok(policy) =
            mochi_privacy::FilePolicy::load(std::path::Path::new(&project.project.root_path))
        else {
            return false;
        };
        policy.fingerprint() == input.file_policy_fingerprint
            && input
                .evidence
                .iter()
                .filter_map(|e| e.path.as_ref())
                .all(|path| {
                    policy
                        .evaluate(
                            std::path::Path::new(path),
                            None,
                            mochi_privacy::Source::Analysis,
                        )
                        .content_allowed()
                })
    }
    pub fn status(&self) -> Result<LocalStatus> {
        Ok(LocalStatus {
            spool_eviction_count: self
                .store
                .spool_eviction_count()
                .map_err(|_| "Local status unavailable.")?,
            schema_version: 1,
            message: self
                .status
                .lock()
                .map_err(|_| "Local status unavailable.")?
                .clone(),
            remote_analysis_enabled: self.analysis.remote_enabled()?,
        })
    }
    pub fn projects(&self, after: Option<ProjectId>) -> Result<Vec<ProjectView>> {
        self.store
            .list_projects(after, 50)
            .map_err(|_| "Projects unavailable.")
            .map(|ps| {
                ps.into_iter()
                    .map(|p| ProjectView {
                        id: p.project.id,
                        name: p.project.display_name,
                        root: p.project.root_path,
                        tracking: p.capture_policy.tracking_enabled,
                        policy_revision: p.capture_policy.revision,
                    })
                    .collect()
            })
    }
    pub fn approve_project(&self, path: String, name: String) -> Result<ProjectView> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        if path.len() > 4096
            || path.chars().any(char::is_control)
            || name.trim().is_empty()
            || name.len() > 80
            || name
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err("Use a short project alias and a specific local folder.");
        }
        let alias = mochi_privacy::RedactionEngine::new()
            .and_then(|r| r.redact_text(name.trim()))
            .map_err(|_| "Project alias unavailable.")?;
        if alias.text != name.trim() {
            return Err("Use a project alias without private credentials.");
        }
        let input = PathBuf::from(path);
        if !input.is_absolute() {
            return Err("Choose an absolute project folder.");
        }
        let root = input
            .canonicalize()
            .map_err(|_| "Project folder unavailable.")?;
        if !root.is_dir()
            || root.parent().is_none()
            || root.components().count() < 4
            || std::env::var_os("HOME")
                .is_some_and(|home| PathBuf::from(home).canonicalize().ok().as_ref() == Some(&root))
        {
            return Err("Choose a specific project folder, not your home or a system root.");
        }
        mochi_privacy::FilePolicy::load(&root)
            .map_err(|_| "Project exclusion policy must be readable and valid.")?;
        let now = UtcTimestamp::parse(&SystemClock.now_rfc3339())
            .map_err(|_| "Local clock unavailable.")?;
        let project = Project {
            id: ProjectId::new(),
            display_name: alias.text,
            root_path: root
                .to_str()
                .ok_or("Project path must be valid text.")?
                .into(),
            repository_identity: None,
            created_at: now.clone(),
            last_seen_at: now,
        };
        self.store
            .create_project(
                &project,
                CapturePolicy {
                    tracking_enabled: false,
                    revision: 1,
                },
            )
            .map_err(|_| "Project is already approved or could not be registered.")?;
        self.bridge
            .reconcile(&self.store, project.id)
            .map_err(|_| "Project registered with capture disabled; policy setup needs retry.")?;
        Ok(ProjectView {
            id: project.id,
            name: project.display_name,
            root: project.root_path,
            tracking: false,
            policy_revision: 1,
        })
    }
    pub fn preview_connection(&self, id: ProjectId, disconnect: bool) -> Result<InstallPreview> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        let p = self
            .store
            .get_project(id)
            .map_err(|_| "Project unavailable.")?
            .filter(|p| p.deleted_at.is_none())
            .ok_or("Project unavailable.")?;
        let plan = if disconnect {
            self.installer
                .prepare_disconnect(&PathBuf::from(&p.project.root_path))
        } else {
            let mut options = CodexDetectorOptions::system(self.helper.clone());
            options.project_roots = vec![PathBuf::from(&p.project.root_path)];
            // Native app launches may lack the user's interactive shell PATH.
            let mut paths = std::env::split_paths(&options.path).collect::<Vec<_>>();
            for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
                let dir = PathBuf::from(dir);
                if !paths.contains(&dir) {
                    paths.push(dir);
                }
            }
            options.path =
                std::env::join_paths(paths).map_err(|_| "Codex discovery unavailable.")?;
            let detection = CodexDetector::new(options).detect();
            if !detection
                .surfaces
                .iter()
                .filter(|s| s.surface == mochi_integration::ClientSurface::Cli)
                .flat_map(|s| &s.installations)
                .any(|i| i.compatibility == Compatibility::Verified)
            {
                return Err(
                    "A validated Codex CLI 0.151.0 installation is required for this test MVP.",
                );
            }
            self.installer.prepare_authorized_install(
                InstallRequest {
                    project_id: id.as_uuid(),
                    approved_root: PathBuf::from(p.project.root_path),
                    helper_path: self.helper.clone(),
                    spool_root: self.spool_root.clone(),
                    policy_revision: p.capture_policy.revision,
                },
                &self.policy_root,
            )
        }
        .map_err(|_| {
            "Could not prepare a safe integration change. Existing hooks may need review."
        })?;
        let preview = plan.preview().clone();
        let mut plans = self.plans.lock().map_err(|_| "Preview unavailable.")?;
        plans.retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        if plans.len() >= 16 {
            plans.clear();
        }
        plans.insert(
            preview.plan_id,
            PendingPlan {
                project_id: id,
                plan,
                created: Instant::now(),
            },
        );
        Ok(preview)
    }
    pub fn apply_connection(&self, token: Uuid, enable_local_capture: bool) -> Result<()> {
        self.analysis.cancel()?;
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        let pending = self
            .plans
            .lock()
            .map_err(|_| "Preview unavailable.")?
            .remove(&token)
            .ok_or("Preview expired. Review the change again.")?;
        if pending.created.elapsed() >= Duration::from_secs(600) {
            return Err("Preview expired. Review the change again.");
        }
        let disconnect =
            pending.plan.preview().action == mochi_integration::InstallAction::Disconnect;
        if !disconnect && !enable_local_capture {
            return Err("Local capture approval is required. No configuration changed.");
        }
        if disconnect {
            self.bridge
                .set_tracking(&self.store, pending.project_id, false)
                .map_err(|_| "Capture could not be disabled safely. Retry disconnect.")?;
            self.store
                .mark_capture_paused(pending.project_id)
                .map_err(|_| "Pause metadata unavailable.")?;
        }
        self.installer.apply(pending.plan, token).map_err(|_| {
            "Configuration changed or integration could not be applied safely. Preview again."
        })?;
        if !disconnect {
            self.bridge
                .set_tracking(&self.store, pending.project_id, true)
                .map_err(|_| {
                    "Hooks installed but capture remains disabled. Retry local capture."
                })?;
        }
        Ok(())
    }
    pub fn tracking(&self, id: ProjectId, enabled: bool) -> Result<()> {
        self.analysis.cancel()?;
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        self.bridge
            .set_tracking(&self.store, id, enabled)
            .map_err(|_| "Capture policy update failed safely. Retry.")?;
        if !enabled {
            self.store
                .mark_capture_paused(id)
                .map_err(|_| "Pause metadata unavailable.")?;
        }
        Ok(())
    }
    pub fn pause_all(&self) -> Result<()> {
        // Each pause completes its own revocation barrier; no enabling is inherited.
        let mut after = None;
        loop {
            let projects = self
                .store
                .list_projects(after, 100)
                .map_err(|_| "Projects unavailable.")?;
            if projects.is_empty() {
                break;
            }
            for p in &projects {
                if p.capture_policy.tracking_enabled {
                    self.tracking(p.project.id, false)?;
                }
            }
            after = projects.last().map(|p| p.project.id);
            if projects.len() < 100 {
                break;
            }
        }
        Ok(())
    }
    fn view(&self, id: SessionId) -> Result<SessionView> {
        let s = self
            .store
            .get_session(id)
            .map_err(|_| "Session unavailable.")?
            .ok_or("Session unavailable.")?;
        let ep = self
            .store
            .get_episode(id)
            .map_err(|_| "Session metadata unavailable.")?;
        Ok(SessionView {
            id,
            started_at: s.data().started_at.clone(),
            ended_at: s.data().ended_at.clone(),
            capture_state: ep
                .as_ref()
                .map(|e| enum_text(&e.state))
                .unwrap_or_else(|| enum_text(&s.data().status)),
            coverage: enum_text(&s.data().capture_completeness.overall()),
            revision: ep.as_ref().map_or(1, |e| e.revision),
            paused: ep.as_ref().is_some_and(|e| e.paused),
            restarted: ep.as_ref().is_some_and(|e| e.restarted),
            late_evidence: ep.as_ref().is_some_and(|e| e.late_evidence),
            continuation_of: ep.as_ref().and_then(|e| e.continuation_id),
        })
    }
    pub fn sessions(&self, id: ProjectId, before: Option<ListCursor>) -> Result<SessionList> {
        let p = self
            .store
            .get_project(id)
            .map_err(|_| "Project unavailable.")?
            .filter(|p| p.deleted_at.is_none())
            .ok_or("Project unavailable.")?;
        let cursor = before.map(|c| SessionCursor {
            started_at: c.started_at,
            session_id: c.session_id,
        });
        let page = self
            .store
            .list_sessions(p.project.id, cursor.as_ref(), Some(50))
            .map_err(|_| "Sessions unavailable.")?;
        Ok(SessionList {
            items: page
                .items
                .iter()
                .map(|s| self.view(s.id))
                .collect::<Result<Vec<_>>>()?,
            next: page.next_cursor.map(|c| ListCursor {
                started_at: c.started_at,
                session_id: c.session_id,
            }),
        })
    }
    pub fn detail(&self, id: SessionId, after: Option<u64>) -> Result<SessionDetail> {
        let s = self
            .store
            .get_session(id)
            .map_err(|_| "Session unavailable.")?
            .ok_or("Session unavailable.")?;
        let data = s.data();
        let mut candidates = data
            .events
            .iter()
            .filter(|e| e.sequence > after.unwrap_or(0))
            .take(51)
            .collect::<Vec<_>>();
        let has_more = candidates.len() > 50;
        candidates.truncate(50);
        let next = if has_more {
            candidates.last().map(|e| e.sequence)
        } else {
            None
        };
        let events = candidates
            .into_iter()
            .map(|e| {
                let value = serde_json::to_value(e).map_err(|_| "Event unavailable.")?;
                let kind = value["eventType"].as_str().unwrap_or("Activity");
                let p = &value["payload"];
                let body = ["text", "display", "output", "summary"]
                    .iter()
                    .find_map(|key| p.get(key).and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        if let Some(exit) = p.get("exitCode") {
                            format!(
                                "Exit code: {}",
                                if exit.is_null() {
                                    "unknown".into()
                                } else {
                                    exit.to_string()
                                }
                            )
                        } else if let Some(status) = p.get("status").or_else(|| p.get("reason")) {
                            status.as_str().unwrap_or("unknown").replace('_', " ")
                        } else {
                            String::new()
                        }
                    });
                let truncated = body.chars().count() > 8192;
                Ok(EventView {
                    id: e.id.to_string(),
                    sequence: e.sequence,
                    title: event_title(kind).into(),
                    text: body.chars().take(8192).collect(),
                    truncated,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let (snapshot,notice)=match &data.git_context {
            mochi_domain::GitContext::Available{before,after} => (after.as_deref().or(Some(before.as_ref())),"Context was captured asynchronously. Earlier changes may be missing; authorship is ambiguous."),
            mochi_domain::GitContext::FinalOnly{after,..}=>(Some(after.as_ref()),"Only the final code is available. The starting state and session delta are unknown."),
            mochi_domain::GitContext::Unavailable{reason}=>(None,match reason{mochi_domain::GitUnavailableReason::NotRepository=>"This project has no available Git repository.",mochi_domain::GitUnavailableReason::NotAuthorized=>"Git collection was not authorized.",_=>"Git context is unavailable or has not been captured."}),
        };
        let code = snapshot
            .map(|s| {
                s.files
                    .iter()
                    .take(100)
                    .map(|f| CodeView {
                        path: f.path.clone(),
                        content: f.content.as_ref().map(|t| t.chars().take(8192).collect()),
                        omitted: f.omission.is_some()
                            || f.content.as_ref().is_some_and(|s| s.chars().count() > 8192),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(SessionDetail {
            session: self.view(id)?,
            events,
            next_sequence: next,
            event_count: data.events.len(),
            code,
            git_notice: notice.into(),
        })
    }
    pub fn finish(&self, id: SessionId) -> Result<()> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        EpisodeEngine::new(&self.store)
            .finish(id, &SystemClock.now_rfc3339())
            .map_err(|_| "Session could not be finalized safely.")
    }
    pub fn delete_session(&self, id: SessionId, confirmed: bool) -> Result<()> {
        self.analysis.cancel()?;
        if !confirmed {
            return Err("Confirm session deletion first.");
        }
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        self.store
            .delete_session(id)
            .map_err(|_| "Session deletion failed.")?;
        self.store
            .checkpoint()
            .map_err(|_| "Session deleted; storage maintenance needs retry.")
    }
    pub fn delete_project(&self, id: ProjectId, confirmed: bool) -> Result<()> {
        self.analysis.cancel()?;
        if !confirmed {
            return Err("Confirm project and session deletion first.");
        }
        let _guard = self
            .operations
            .lock()
            .map_err(|_| "Local operation unavailable.")?;
        self.bridge
            .delete_project(&self.store, id)
            .map_err(|_| "Project deletion failed safely.")?;
        self.store
            .checkpoint()
            .map_err(|_| "Project deleted; storage maintenance needs retry.")
    }
}
fn enum_text(value: &impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}
fn event_title(kind: &str) -> &str {
    match kind {
        "session.started" => "Session started",
        "user.prompt" => "Your request",
        "agent.message" => "Codex response",
        "tool.started" => "Tool started",
        "tool.completed" => "Tool finished",
        "command.executed" => "Command observed",
        "command.result" => "Command result",
        "turn.completed" => "Turn ended",
        "session.stopped" => "Session boundary",
        "permission.requested" => "Permission requested",
        "context.compacted" => "Context compacted",
        "capture.gap" => "Capture gap",
        _ => "Observed activity",
    }
}
