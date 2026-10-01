mod lifecycle;
pub use lifecycle::{EpisodeEngine, EpisodeRun};
mod engine;
mod error;
mod reconstruct;

pub use engine::{AssemblyLimits, AssemblyRunOutcome, ExplicitGitContext, SessionAssemblyEngine};
pub use error::{AssemblyError, AssemblyResult};
pub use reconstruct::{
    ASSEMBLY_VERSION, assemble_episode, assemble_session, assembly_group_key, evidence_fingerprint,
    session_id_for_source,
};

#[cfg(test)]
mod tests;
