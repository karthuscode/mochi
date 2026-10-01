mod learning;
pub use learning::{AnalysisRun, LearningAttempt, LearningQuestion};
mod episode;
pub use episode::{CaptureEpisode, EpisodeState};
mod assembly;
mod connection;
mod error;
mod ingress;
mod migrations;
mod project;
mod session;

pub use connection::{Clock, SqliteStore, SystemClock};
pub use error::{StorageError, StorageResult};
pub use ingress::{ImportOutcome, IngressCursor, IngressPage, IngressRepository, PersistedIngress};
pub use project::{CapturePolicy, ProjectRepository, StoredProject};
pub use session::{CodingSessionRepository, SessionCursor, SessionPage, SessionSummary};

#[cfg(test)]
mod tests;
pub use assembly::{
    AssemblyCandidate, AssemblyEvidenceLoad, AssemblyRepository, AssemblySourceKey, AssemblyWrite,
    AssemblyWriteOutcome, DEFAULT_ASSEMBLY_CANDIDATES, IgnoredIngressReason, IngressAssemblyState,
    MAX_ASSEMBLY_BYTES, MAX_ASSEMBLY_CANDIDATES, MAX_ASSEMBLY_EVENTS,
};
