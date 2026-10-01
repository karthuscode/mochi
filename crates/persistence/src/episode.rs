use crate::{
    AssemblySourceKey, CodingSessionRepository, MAX_ASSEMBLY_BYTES, MAX_ASSEMBLY_EVENTS,
    PersistedIngress, SqliteStore, StorageError, StorageResult,
    error::map_sqlite,
    ingress::{decode_ingress, decode_ingress_row},
    session::replace_session_transaction,
};
use mochi_domain::{CodingSession, ProjectId, SessionId};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeState {
    Active,
    Idle,
    Interrupted,
    Finalized,
    Deleted,
}
impl EpisodeState {
    pub fn is_open(&self) -> bool {
        matches!(self, Self::Active | Self::Idle | Self::Interrupted)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureEpisode {
    pub id: SessionId,
    pub source: AssemblySourceKey,
    pub source_key: String,
    pub continuation_id: Option<SessionId>,
    pub state: EpisodeState,
    pub revision: u64,
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub last_observed_at: String,
    pub finalized_at: Option<String>,
    pub end_reason: Option<String>,
    pub paused: bool,
    pub restarted: bool,
    pub late_evidence: bool,
    pub fingerprint: String,
}
impl SqliteStore {
    pub fn episode_sources(&self) -> StorageResult<Vec<AssemblySourceKey>> {
        let c = self.lock()?;
        let mut stmt=c.prepare("SELECT e.project_id,e.provider,e.adapter_version,e.transport,e.client_surface,e.external_session_id FROM ingress_events e WHERE e.external_session_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM episode_ingress a WHERE a.ingress_id=e.ingress_id) AND NOT EXISTS(SELECT 1 FROM ingress_assembly_state a WHERE a.ingress_id=e.ingress_id) GROUP BY e.project_id,e.provider,e.adapter_version,e.transport,e.client_surface,e.external_session_id ORDER BY MIN(e.receive_sequence) LIMIT 25").map_err(map_sqlite)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })
            .map_err(map_sqlite)?;
        let mut sources = Vec::new();
        for row in rows {
            let r = row.map_err(map_sqlite)?;
            sources.push(AssemblySourceKey {
                project_id: ProjectId::parse(&r.0).map_err(|_| StorageError::Corrupt)?,
                provider: r.1,
                adapter_version: r.2,
                transport: r.3,
                client_surface: mochi_capture::ClientSurface::parse(&r.4)
                    .ok_or(StorageError::Corrupt)?,
                external_session_id: r.5,
            });
        }
        Ok(sources)
    }
    pub fn latest_episode(&self, key: &str) -> StorageResult<Option<CaptureEpisode>> {
        let id:Option<String>=self.lock()?.query_row("SELECT id FROM capture_episodes WHERE source_key=?1 ORDER BY first_sequence DESC LIMIT 1",[key],|r|r.get(0)).optional().map_err(map_sqlite)?;
        id.map(|s| self.get_episode(SessionId::parse(&s).map_err(|_| StorageError::Corrupt)?))
            .transpose()
            .map(Option::flatten)
    }
    pub fn get_episode(&self, id: SessionId) -> StorageResult<Option<CaptureEpisode>> {
        self.lock()?.query_row("SELECT source_json,source_key,continuation_id,state,revision,first_sequence,last_sequence,last_observed_at,finalized_at,end_reason,paused,restarted,late_evidence,fingerprint FROM capture_episodes WHERE id=?1",[id.to_string()],|r| {
            Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,r.get::<_,i64>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,String>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,bool>(10)?,r.get::<_,bool>(11)?,r.get::<_,bool>(12)?,r.get::<_,String>(13)?))
        }).optional().map_err(map_sqlite)?.map(|r|Ok(CaptureEpisode { id,source:serde_json::from_str(&r.0).map_err(|_|StorageError::Corrupt)?,source_key:r.1,continuation_id:r.2.map(|s|SessionId::parse(&s).map_err(|_|StorageError::Corrupt)).transpose()?,state:serde_json::from_value(serde_json::Value::String(r.3)).map_err(|_|StorageError::Corrupt)?,revision:u64::try_from(r.4).map_err(|_|StorageError::Corrupt)?,first_sequence:u64::try_from(r.5).map_err(|_|StorageError::Corrupt)?,last_sequence:u64::try_from(r.6).map_err(|_|StorageError::Corrupt)?,last_observed_at:r.7,finalized_at:r.8,end_reason:r.9,paused:r.10,restarted:r.11,late_evidence:r.12,fingerprint:r.13})).transpose()
    }
    pub fn episode_evidence(
        &self,
        source: &AssemblySourceKey,
        id: Option<SessionId>,
    ) -> StorageResult<Vec<PersistedIngress>> {
        let c = self.lock()?;
        let mut stmt=c.prepare("SELECT e.ingress_id,e.source_identity_key,e.schema_version,e.provider,e.adapter_version,e.transport,e.client_surface,e.source_event_id,e.source_sequence,e.source_timestamp,e.received_at,e.receive_sequence,e.source_event_type,e.external_session_id,e.external_turn_id,e.external_tool_use_id,e.origin,e.normalized_event_json,e.sensitivity_json,e.imported_at FROM ingress_events e LEFT JOIN episode_ingress a ON a.ingress_id=e.ingress_id WHERE e.project_id=?1 AND e.provider=?2 AND e.adapter_version=?3 AND e.transport=?4 AND e.client_surface=?5 AND e.external_session_id=?6 AND (a.episode_id=?7 OR (a.ingress_id IS NULL AND NOT EXISTS(SELECT 1 FROM ingress_assembly_state old WHERE old.ingress_id=e.ingress_id))) ORDER BY e.receive_sequence,e.ingress_id LIMIT 20001").map_err(map_sqlite)?;
        let surface =
            serde_json::to_value(source.client_surface).map_err(|_| StorageError::Serialization)?;
        let rows = stmt
            .query_map(
                params![
                    source.project_id.to_string(),
                    source.provider,
                    source.adapter_version,
                    source.transport,
                    surface.as_str(),
                    source.external_session_id,
                    id.map(|id| id.to_string())
                ],
                decode_ingress_row,
            )
            .map_err(map_sqlite)?;
        let mut items = Vec::new();
        let mut bytes = 0;
        for r in rows {
            let item = decode_ingress(source.project_id, r.map_err(map_sqlite)?)?;
            bytes += serde_json::to_vec(&item.record)
                .map_err(|_| StorageError::Serialization)?
                .len();
            if items.len() >= MAX_ASSEMBLY_EVENTS || bytes > MAX_ASSEMBLY_BYTES {
                return Err(StorageError::InvalidInput);
            }
            items.push(item);
        }
        Ok(items)
    }
    /// Header, validated aggregate and immutable ingress ownership commit together.
    pub fn write_episode(
        &self,
        ep: &CaptureEpisode,
        session: &CodingSession,
        evidence: &[PersistedIngress],
        expected_revision: u64,
    ) -> StorageResult<()> {
        if evidence.is_empty()
            || evidence.len() > MAX_ASSEMBLY_EVENTS
            || ep.id != session.data().id
            || ep.source.project_id != session.data().project_id
            || Some(ep.revision) != expected_revision.checked_add(1)
            || ep.state == EpisodeState::Deleted
            || evidence.first().map(|r| r.record.receive_sequence) != Some(ep.first_sequence)
            || evidence.last().map(|r| r.record.receive_sequence) != Some(ep.last_sequence)
        {
            return Err(StorageError::InvalidInput);
        }
        let unique = evidence.iter().map(|r| r.record.id).collect::<HashSet<_>>();
        if unique.len() != evidence.len() {
            return Err(StorageError::InvalidInput);
        }
        let now = self.now()?;
        let mut c = self.lock()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        let prior: Option<(i64, String)> = tx
            .query_row(
                "SELECT revision,state FROM capture_episodes WHERE id=?1",
                [ep.id.to_string()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(map_sqlite)?;
        if prior.as_ref().map_or(expected_revision != 0, |p| {
            u64::try_from(p.0).ok() != Some(expected_revision) || p.1 == "deleted"
        }) {
            return Err(StorageError::Constraint);
        }
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND deleted_at IS NULL)",
                [ep.source.project_id.to_string()],
                |r| r.get(0),
            )
            .map_err(map_sqlite)?;
        if !active {
            return Err(StorageError::PolicyRejected);
        }
        for item in evidence {
            let r = &item.record;
            if r.project_id != ep.source.project_id.as_uuid()
                || r.external_session_id.as_deref() != Some(&ep.source.external_session_id)
                || r.source.provider != ep.source.provider
                || r.source.adapter_version != ep.source.adapter_version
                || r.source.transport != ep.source.transport
                || r.source.client_surface != ep.source.client_surface
            {
                return Err(StorageError::InvalidInput);
            }
            let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ingress_events e WHERE e.ingress_id=?1 AND e.project_id=?2 AND e.receive_sequence=?3 AND NOT EXISTS(SELECT 1 FROM episode_ingress a WHERE a.ingress_id=e.ingress_id AND a.episode_id<>?4) AND NOT EXISTS(SELECT 1 FROM ingress_assembly_state a WHERE a.ingress_id=e.ingress_id))",params![r.id.to_string(),ep.source.project_id.to_string(),number(r.receive_sequence)?,ep.id.to_string()],|r|r.get(0)).map_err(map_sqlite)?;
            if !valid {
                return Err(StorageError::Constraint);
            }
        }
        // Only this revisioned path may replace an aggregate; public insert remains immutable.
        replace_session_transaction(&tx, session, &now)?;
        let state = serde_json::to_string(&ep.state).map_err(|_| StorageError::Serialization)?;
        tx.execute("INSERT INTO capture_episodes(id,project_id,source_key,source_json,continuation_id,state,revision,first_sequence,last_sequence,last_observed_at,finalized_at,end_reason,paused,restarted,late_evidence,fingerprint) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) ON CONFLICT(id) DO UPDATE SET state=excluded.state,revision=excluded.revision,last_sequence=excluded.last_sequence,last_observed_at=excluded.last_observed_at,finalized_at=excluded.finalized_at,end_reason=excluded.end_reason,paused=excluded.paused,restarted=excluded.restarted,late_evidence=excluded.late_evidence,fingerprint=excluded.fingerprint",params![ep.id.to_string(),ep.source.project_id.to_string(),ep.source_key,serde_json::to_string(&ep.source).map_err(|_|StorageError::Serialization)?,ep.continuation_id.map(|id|id.to_string()),state.trim_matches('"'),number(ep.revision)?,number(ep.first_sequence)?,number(ep.last_sequence)?,ep.last_observed_at,ep.finalized_at,ep.end_reason,ep.paused,ep.restarted,ep.late_evidence,ep.fingerprint]).map_err(map_sqlite)?;
        for item in evidence {
            tx.execute("INSERT INTO episode_ingress(ingress_id,episode_id,receive_sequence) VALUES(?1,?2,?3) ON CONFLICT(ingress_id) DO NOTHING",params![item.record.id.to_string(),ep.id.to_string(),number(item.record.receive_sequence)?]).map_err(map_sqlite)?;
        }
        tx.commit().map_err(map_sqlite)
    }
    pub fn next_git_episode(&self) -> StorageResult<Option<SessionId>> {
        let id:Option<String>=self.lock()?.query_row("SELECT e.id FROM capture_episodes e JOIN projects p ON p.id=e.project_id WHERE p.tracking_enabled=1 AND p.deleted_at IS NULL AND e.state<>'deleted' AND (EXISTS(SELECT 1 FROM git_snapshots g WHERE g.session_id=e.id AND g.role='unavailable' AND json_extract(g.row_json,'$.availability')='unavailable' AND json_extract(g.row_json,'$.reason')='not_captured') OR (e.state='finalized' AND EXISTS(SELECT 1 FROM git_snapshots g WHERE g.session_id=e.id AND g.role='before') AND NOT EXISTS(SELECT 1 FROM git_snapshots g WHERE g.session_id=e.id AND g.role='after'))) ORDER BY e.first_sequence LIMIT 1",[],|r|r.get(0)).optional().map_err(map_sqlite)?;
        id.map(|s| SessionId::parse(&s).map_err(|_| StorageError::Corrupt))
            .transpose()
    }
    pub fn mark_capture_restart(&self) -> StorageResult<()> {
        self.lock()?.execute("UPDATE capture_episodes SET state='interrupted',restarted=1,revision=revision+1 WHERE state IN ('active','idle')",[]).map_err(map_sqlite)?;
        Ok(())
    }
    pub fn mark_capture_paused(&self, id: ProjectId) -> StorageResult<()> {
        self.lock()?.execute("UPDATE capture_episodes SET paused=1,state='interrupted',revision=revision+1 WHERE project_id=?1 AND state IN ('active','idle','interrupted')",[id.to_string()]).map_err(map_sqlite)?;
        Ok(())
    }
    pub fn finalized_session(&self, id: SessionId) -> StorageResult<Option<CodingSession>> {
        if self
            .get_episode(id)?
            .is_some_and(|e| e.state == EpisodeState::Finalized)
        {
            self.get_session(id)
        } else {
            Ok(None)
        }
    }
}

fn number(v: u64) -> StorageResult<i64> {
    i64::try_from(v).map_err(|_| StorageError::InvalidInput)
}
