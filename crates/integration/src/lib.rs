//! Read-only coding-agent installation and capability detection.

mod codex;
mod model;
mod process;

pub use codex::{CodexDetector, CodexDetectorOptions};
pub use model::*;

/// Provider-independent boundary for native coding-agent detection.
pub trait IntegrationDetectionService {
    fn detect(&self) -> IntegrationDetection;
}

#[cfg(test)]
mod tests;
