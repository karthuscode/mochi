//! Production episode orchestration; turn stops never imply session completion.
use crate::{
    AssemblyError, AssemblyResult, assemble_episode, assembly_group_key, evidence_fingerprint,
};
use mochi_capture::model::{NormalizedEvent, SessionStartReason};
use mochi_domain::{CodingSession, EvidenceCompleteness, SessionId, SessionStatus, UtcTimestamp};
use mochi_persistence::{
    AssemblyRepository, CaptureEpisode, CodingSessionRepository, EpisodeState,
    IgnoredIngressReason, PersistedIngress, SqliteStore,
};

#[derive(Default, Debug)]
pub struct EpisodeRun {
    pub updated: usize,
    pub deferred: usize,
}
pub struct EpisodeEngine<'a> {
    store: &'a SqliteStore,
}
impl<'a> EpisodeEngine<'a> {
    pub fn new(store: &'a SqliteStore) -> Self {
        Self { store }
    }
    pub fn process_pending(&self) -> AssemblyResult<EpisodeRun> {
        let mut outcome = EpisodeRun::default();
        let unidentified = self.store.list_unidentified_ingress(100)?;
        if !unidentified.is_empty() {
            self.store.mark_ingress_ignored(
                &unidentified.iter().map(|r| r.record.id).collect::<Vec<_>>(),
                IgnoredIngressReason::MissingSessionIdentity,
                2,
            )?;
        }
        for source in self.store.episode_sources()? {
            match self.process_source(&source) {
                Ok(true) => outcome.updated += 1,
                Ok(false) => {}
                Err(_) => outcome.deferred += 1,
            }
        }
        Ok(outcome)
    }
    fn process_source(
        &self,
        source: &mochi_persistence::AssemblySourceKey,
    ) -> AssemblyResult<bool> {
        let pending = self.store.episode_evidence(source, None)?;
        if pending.is_empty() {
            return Ok(false);
        }
        let key = assembly_group_key(source);
        let mut previous = self.store.latest_episode(&key)?;
        if let Some(old) = previous.as_ref() {
            if old.state == EpisodeState::Deleted && !starts_episode(&pending[0]) {
                self.store.mark_ingress_ignored(
                    &pending.iter().map(|r| r.record.id).collect::<Vec<_>>(),
                    IgnoredIngressReason::DeletedSession,
                    2,
                )?;
                return Ok(false);
            }
            if old.restarted
                && old.state.is_open()
                && !matches!(&pending[0].record.event,NormalizedEvent::SessionStarted(p) if p.reason==SessionStartReason::Resume)
            {
                self.close(old.id, &old.last_observed_at, "restart_boundary_unknown")?;
                previous = self.store.latest_episode(&key)?;
            }
        }
        let continuation = previous
            .as_ref()
            .filter(|old| !old.state.is_open() && starts_episode(&pending[0]))
            .map(|old| old.id);
        let reuse = previous.as_ref().filter(|old| {
            old.state != EpisodeState::Deleted && (old.state.is_open() || continuation.is_none())
        });
        let mut ep = if let Some(old) = reuse {
            old.clone()
        } else {
            CaptureEpisode {
                id: SessionId::parse(&pending[0].record.id.to_string())
                    .map_err(|_| AssemblyError::DomainValidation)?,
                source: source.clone(),
                source_key: key,
                continuation_id: previous.as_ref().map(|old| old.id),
                state: EpisodeState::Active,
                revision: 0,
                first_sequence: pending[0].record.receive_sequence,
                last_sequence: pending[0].record.receive_sequence,
                last_observed_at: pending[0].record.received_at.clone(),
                finalized_at: None,
                end_reason: None,
                paused: false,
                restarted: false,
                late_evidence: false,
                fingerprint: String::new(),
            }
        };
        let mut evidence = if reuse.is_some() {
            self.store.episode_evidence(source, Some(ep.id))?
        } else {
            pending
        };
        let old_last = if reuse.is_some() { ep.last_sequence } else { 0 };
        if let Some(pos)=evidence.iter().position(|r|r.record.receive_sequence>old_last && matches!(&r.record.event,NormalizedEvent::SessionStopped(s) if s.end_boundary_known)) { evidence.truncate(pos+1); }
        let last = evidence
            .last()
            .ok_or(AssemblyError::InvalidEvidenceRelationship)?;
        if ep.state == EpisodeState::Finalized {
            ep.late_evidence = true;
        } else {
            ep.state=if evidence.iter().any(|r|matches!(&r.record.event,NormalizedEvent::SessionStopped(s) if s.end_boundary_known)){EpisodeState::Finalized}
                else if matches!(last.record.event,NormalizedEvent::TurnCompleted(_)){EpisodeState::Idle}else{EpisodeState::Active};
            if ep.state == EpisodeState::Finalized {
                ep.finalized_at = Some(last.record.received_at.clone());
                ep.end_reason = Some("provider_boundary_unknown_reason".into());
            }
        }
        ep.first_sequence = evidence[0].record.receive_sequence;
        ep.last_sequence = last.record.receive_sequence;
        ep.last_observed_at = last.record.received_at.clone();
        self.persist(ep, &evidence, None)?;
        Ok(true)
    }
    pub fn finish(&self, id: SessionId, now: &str) -> AssemblyResult<()> {
        self.close(id, now, "user_finalized")
    }
    fn close(&self, id: SessionId, now: &str, reason: &str) -> AssemblyResult<()> {
        UtcTimestamp::parse(now).map_err(|_| AssemblyError::DomainValidation)?;
        let mut ep = self
            .store
            .get_episode(id)?
            .ok_or(AssemblyError::InvalidEvidenceRelationship)?;
        if ep.state == EpisodeState::Deleted {
            return Err(AssemblyError::InvalidEvidenceRelationship);
        }
        if ep.state == EpisodeState::Finalized && ep.end_reason.as_deref() == Some(reason) {
            return Ok(());
        }
        let evidence = self
            .store
            .episode_evidence(&ep.source, Some(id))?
            .into_iter()
            .filter(|r| r.record.receive_sequence <= ep.last_sequence)
            .collect::<Vec<_>>();
        ep.state = EpisodeState::Finalized;
        ep.finalized_at = Some(now.into());
        ep.end_reason = Some(reason.into());
        if let Some(last) = evidence.last() {
            ep.last_sequence = last.record.receive_sequence;
            ep.last_observed_at = last.record.received_at.clone();
        }
        self.persist(ep, &evidence, None)
    }
    pub fn set_git_context(
        &self,
        id: SessionId,
        context: &mochi_domain::GitContext,
    ) -> AssemblyResult<()> {
        context
            .validate()
            .map_err(|_| AssemblyError::DomainValidation)?;
        let ep = self
            .store
            .get_episode(id)?
            .ok_or(AssemblyError::InvalidEvidenceRelationship)?;
        let evidence = self
            .store
            .episode_evidence(&ep.source, Some(id))?
            .into_iter()
            .filter(|r| r.record.receive_sequence <= ep.last_sequence)
            .collect::<Vec<_>>();
        self.persist(ep, &evidence, Some(context))
    }
    fn persist(
        &self,
        mut ep: CaptureEpisode,
        evidence: &[PersistedIngress],
        git_override: Option<&mochi_domain::GitContext>,
    ) -> AssemblyResult<()> {
        let prior = self.store.get_session(ep.id)?;
        let git = git_override.or_else(|| prior.as_ref().map(|s| &s.data().git_context));
        let session = assemble_episode(&ep.source, evidence, git, ep.id)?;
        let mut data = session.into_data();
        if ep.state.is_open() {
            data.status = SessionStatus::Active;
            data.ended_at = None;
        } else if ep.end_reason.as_deref() != Some("provider_boundary_unknown_reason") {
            data.status = SessionStatus::Incomplete;
            data.ended_at = ep
                .finalized_at
                .as_deref()
                .map(UtcTimestamp::parse)
                .transpose()
                .map_err(|_| AssemblyError::DomainValidation)?;
            data.capture_completeness.session_lifecycle = EvidenceCompleteness::Partial;
        }
        if ep.paused || ep.restarted || ep.late_evidence {
            data.capture_completeness.session_lifecycle = EvidenceCompleteness::Partial;
        }
        let session = CodingSession::new(data).map_err(|_| AssemblyError::DomainValidation)?;
        let expected = ep.revision;
        ep.revision = expected
            .checked_add(1)
            .ok_or(AssemblyError::BoundsExceeded)?;
        ep.fingerprint = evidence_fingerprint(evidence);
        self.store
            .write_episode(&ep, &session, evidence, expected)?;
        Ok(())
    }
}
fn starts_episode(e: &PersistedIngress) -> bool {
    matches!(
        e.record.event,
        NormalizedEvent::SessionStarted(_) | NormalizedEvent::UserPrompt(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mochi_capture::{CaptureSanitizer, CodexSessionSource, SessionSource, Spool, SpoolLimits};
    use mochi_domain::{Project, ProjectId};
    use mochi_persistence::{CapturePolicy, IngressRepository, ProjectRepository};
    struct FixedClock;
    impl mochi_capture::source::Clock for FixedClock {
        fn now_rfc3339(&self) -> String {
            "2026-10-01T12:00:00Z".into()
        }
    }
    struct Fixture {
        _temp: tempfile::TempDir,
        store: SqliteStore,
        source: CodexSessionSource<CaptureSanitizer, FixedClock>,
        spool: Spool,
        root: std::path::PathBuf,
    }
    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::create_dir(root.join("project")).unwrap();
        let store = SqliteStore::open_system(root.join("data/mochi.sqlite3")).unwrap();
        let id = ProjectId::new();
        let now = UtcTimestamp::parse("2026-10-01T12:00:00Z").unwrap();
        store
            .create_project(
                &Project {
                    id,
                    display_name: "Synthetic".into(),
                    root_path: root.join("project").to_string_lossy().into(),
                    repository_identity: None,
                    created_at: now.clone(),
                    last_seen_at: now,
                },
                CapturePolicy {
                    tracking_enabled: true,
                    revision: 1,
                },
            )
            .unwrap();
        let source = CodexSessionSource::new(
            id.as_uuid(),
            root.join("project"),
            mochi_capture::ClientSurface::Cli,
            1,
            CaptureSanitizer::new(&root.join("project")).unwrap(),
            FixedClock,
        );
        Fixture {
            _temp: temp,
            store,
            source,
            spool: Spool::new(root.join("spool"), SpoolLimits::default()),
            root,
        }
    }
    fn send(f: &Fixture, event: &str, turn: &str) {
        let json = serde_json::json!({"hook_event_name":event,"cwd":f.root.join("project"),"session_id":"synthetic-episode","turn_id":turn,"source":"startup","prompt":"Explain sum","last_assistant_message":"A sum returns addition"});
        f.spool
            .append(
                f.source.project_id(),
                f.source.descriptor(),
                "2026-10-01T12:00:00Z",
                f.source
                    .normalize(&serde_json::to_vec(&json).unwrap())
                    .unwrap(),
            )
            .unwrap();
        f.store.import_spool_batch(&f.spool, 100).unwrap();
    }
    #[test]
    fn live_idle_finalize_continuation_and_late_revision() {
        let f = fixture();
        let engine = EpisodeEngine::new(&f.store);
        send(&f, "SessionStart", "t1");
        assert_eq!(engine.process_pending().unwrap().updated, 1);
        let source = f.store.episode_sources().unwrap();
        assert!(source.is_empty());
        let page = f
            .store
            .list_sessions(
                ProjectId::parse(&f.source.project_id().to_string()).unwrap(),
                None,
                None,
            )
            .unwrap();
        let id = page.items[0].id;
        let first = f.store.get_episode(id).unwrap().unwrap();
        assert_eq!(first.state, EpisodeState::Active);
        send(&f, "UserPromptSubmit", "t1");
        send(&f, "Stop", "t1");
        assert_eq!(engine.process_pending().unwrap().updated, 1);
        assert_eq!(
            f.store.get_episode(id).unwrap().unwrap().state,
            EpisodeState::Idle
        );
        assert_eq!(
            f.store.get_session(id).unwrap().unwrap().data().status,
            SessionStatus::Active
        );
        send(&f, "SessionEnd", "t1");
        assert_eq!(engine.process_pending().unwrap().updated, 1);
        assert_eq!(
            f.store.get_episode(id).unwrap().unwrap().state,
            EpisodeState::Finalized
        );
        send(&f, "UserPromptSubmit", "t2");
        engine.process_pending().unwrap();
        let next = f.store.latest_episode(&first.source_key).unwrap().unwrap();
        assert_ne!(id, next.id);
        assert_eq!(next.continuation_id, Some(id));
        engine.finish(next.id, "2026-10-01T12:01:00Z").unwrap();
        send(&f, "Stop", "t2");
        engine.process_pending().unwrap();
        let amended = f.store.get_episode(next.id).unwrap().unwrap();
        assert!(amended.late_evidence);
        assert!(amended.revision > next.revision);
        let before = amended.revision;
        assert_eq!(engine.process_pending().unwrap().updated, 0);
        assert_eq!(
            f.store.get_episode(next.id).unwrap().unwrap().revision,
            before
        );
    }
    #[test]
    fn restart_pause_and_delete_never_recreate_old_aggregate() {
        let f = fixture();
        let engine = EpisodeEngine::new(&f.store);
        send(&f, "UserPromptSubmit", "t1");
        engine.process_pending().unwrap();
        let source_key = f
            .store
            .get_episode(
                f.store
                    .list_sessions(
                        ProjectId::parse(&f.source.project_id().to_string()).unwrap(),
                        None,
                        None,
                    )
                    .unwrap()
                    .items[0]
                    .id,
            )
            .unwrap()
            .unwrap()
            .source_key;
        let old = f.store.latest_episode(&source_key).unwrap().unwrap();
        f.store.mark_capture_restart().unwrap();
        assert_eq!(
            f.store.get_episode(old.id).unwrap().unwrap().state,
            EpisodeState::Interrupted
        );
        send(&f, "UserPromptSubmit", "t2");
        assert_eq!(engine.process_pending().unwrap().updated, 1);
        let next = f.store.latest_episode(&source_key).unwrap().unwrap();
        assert_ne!(next.id, old.id);
        assert_eq!(next.continuation_id, Some(old.id));
        f.store.mark_capture_paused(next.source.project_id).unwrap();
        engine.finish(next.id, "2026-10-01T12:02:00Z").unwrap();
        assert!(f.store.get_episode(next.id).unwrap().unwrap().paused);
        let owned = f
            .store
            .page_ingress(next.source.project_id, None, Some(100))
            .unwrap()
            .items;
        let replay = owned
            .iter()
            .find(|e| e.record.external_turn_id.as_deref() == Some("t2"))
            .unwrap()
            .record
            .clone();
        f.store.delete_session(next.id).unwrap();
        let retained = f
            .store
            .page_ingress(next.source.project_id, None, Some(100))
            .unwrap()
            .items;
        assert!(retained.iter().all(|e| e.record.id != replay.id));
        let replay_path = f.root.join("spool").join(format!(
            "{:020}-{}.json",
            replay.receive_sequence, replay.id
        ));
        std::fs::write(&replay_path, serde_json::to_vec(&replay).unwrap()).unwrap();
        assert_eq!(
            f.store
                .import_spool_batch(&f.spool, 100)
                .unwrap()
                .tombstoned,
            1
        );
        assert!(!replay_path.exists());
        send(&f, "Stop", "t2");
        engine.process_pending().unwrap();
        assert!(f.store.get_session(next.id).unwrap().is_none());
        assert_eq!(
            f.store.get_episode(next.id).unwrap().unwrap().state,
            EpisodeState::Deleted
        );
    }
}
