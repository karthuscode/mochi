use crate::{
    ASSEMBLY_VERSION, AssemblyError, AssemblyResult, assemble_session, assembly_group_key,
    evidence_fingerprint,
};
use mochi_domain::{GitContext, SessionId};
use mochi_persistence::{
    AssemblyEvidenceLoad, AssemblyRepository, AssemblySourceKey, AssemblyWrite,
    AssemblyWriteOutcome, IgnoredIngressReason, MAX_ASSEMBLY_BYTES, MAX_ASSEMBLY_CANDIDATES,
    MAX_ASSEMBLY_EVENTS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssemblyLimits {
    pub candidate_sessions: usize,
    pub ingress_records_per_session: usize,
    pub serialized_bytes_per_session: usize,
    pub unidentified_records: usize,
}

impl Default for AssemblyLimits {
    fn default() -> Self {
        Self {
            candidate_sessions: 10,
            ingress_records_per_session: MAX_ASSEMBLY_EVENTS,
            serialized_bytes_per_session: MAX_ASSEMBLY_BYTES,
            unidentified_records: 100,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitGitContext {
    pub source: AssemblySourceKey,
    pub context: GitContext,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AssemblyRunOutcome {
    pub assembled_session_ids: Vec<SessionId>,
    pub existing_sessions: usize,
    pub deleted_sessions_skipped: usize,
    pub ignored_ingress: usize,
    pub deferred_groups: usize,
}

pub struct SessionAssemblyEngine<'a, R> {
    repository: &'a R,
}

impl<'a, R: AssemblyRepository> SessionAssemblyEngine<'a, R> {
    pub fn new(repository: &'a R) -> Self {
        Self { repository }
    }

    pub fn assemble_pending(
        &self,
        limits: AssemblyLimits,
        git_contexts: &[ExplicitGitContext],
    ) -> AssemblyResult<AssemblyRunOutcome> {
        validate_limits(limits)?;
        let mut outcome = AssemblyRunOutcome::default();

        let unidentified = self
            .repository
            .list_unidentified_ingress(limits.unidentified_records)?;
        if !unidentified.is_empty() {
            let ids = unidentified
                .iter()
                .map(|value| value.record.id)
                .collect::<Vec<_>>();
            outcome.ignored_ingress += self.repository.mark_ingress_ignored(
                &ids,
                IgnoredIngressReason::MissingSessionIdentity,
                ASSEMBLY_VERSION,
            )?;
        }

        let candidates = self
            .repository
            .list_assembly_candidates(Some(limits.candidate_sessions))?;
        for candidate in candidates {
            if candidate.record_count > limits.ingress_records_per_session
                || candidate.serialized_bytes > limits.serialized_bytes_per_session
            {
                outcome.deferred_groups += 1;
                continue;
            }
            let evidence = match self.repository.load_assembly_evidence(
                &candidate.source,
                limits.ingress_records_per_session,
                limits.serialized_bytes_per_session,
            )? {
                AssemblyEvidenceLoad::Ready(evidence) => evidence,
                AssemblyEvidenceLoad::BoundsExceeded { .. } => {
                    outcome.deferred_groups += 1;
                    continue;
                }
            };
            if evidence.is_empty() {
                continue;
            }
            let git_context = git_contexts
                .iter()
                .find(|value| value.source == candidate.source)
                .map(|value| &value.context);
            let session = assemble_session(&candidate.source, &evidence, git_context)?;
            let output_bytes = session
                .to_json()
                .map_err(|_| AssemblyError::DomainValidation)?
                .len();
            if output_bytes > limits.serialized_bytes_per_session {
                outcome.deferred_groups += 1;
                continue;
            }
            let ingress_ids = evidence
                .iter()
                .map(|value| value.record.id)
                .collect::<Vec<_>>();
            let group_key = assembly_group_key(&candidate.source);
            let fingerprint = evidence_fingerprint(&evidence);
            match self.repository.persist_assembled_session(AssemblyWrite {
                group_key: &group_key,
                source: &candidate.source,
                session: &session,
                ingress_ids: &ingress_ids,
                assembly_version: ASSEMBLY_VERSION,
                evidence_fingerprint: &fingerprint,
            })? {
                AssemblyWriteOutcome::Inserted => {
                    outcome.assembled_session_ids.push(session.data().id);
                }
                AssemblyWriteOutcome::Existing(_) => outcome.existing_sessions += 1,
                AssemblyWriteOutcome::Deleted(_) => outcome.deleted_sessions_skipped += 1,
            }
        }
        Ok(outcome)
    }
}

fn validate_limits(limits: AssemblyLimits) -> AssemblyResult<()> {
    if limits.candidate_sessions == 0
        || limits.candidate_sessions > MAX_ASSEMBLY_CANDIDATES
        || limits.ingress_records_per_session == 0
        || limits.ingress_records_per_session > MAX_ASSEMBLY_EVENTS
        || limits.serialized_bytes_per_session == 0
        || limits.serialized_bytes_per_session > MAX_ASSEMBLY_BYTES
        || limits.unidentified_records == 0
        || limits.unidentified_records > 100
    {
        return Err(AssemblyError::BoundsExceeded);
    }
    Ok(())
}
