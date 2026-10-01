pub mod authorization;
pub mod git;
pub mod model;
pub mod sanitize;
pub mod source;
pub mod spool;

pub use git::{GitCliContextReader, GitContextReader, GitSnapshot, SnapshotRole};
pub use model::{ClientSurface, SpoolIngressRecord};
pub use sanitize::{CaptureSanitizer, PrototypeSanitizer};
pub use source::{CodexSessionSource, IntakeError, SessionSource, SystemClock};
pub use spool::{Spool, SpoolError, SpoolLimits};

#[cfg(test)]
mod tests;
