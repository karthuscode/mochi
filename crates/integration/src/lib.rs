//! Read-only coding-agent installation and capability detection.

mod codex;
mod hooks;
mod installer;
mod model;
mod process;

pub use codex::{CodexDetector, CodexDetectorOptions};
pub use hooks::CAPTURE_EVENTS;
pub use installer::*;
pub use model::*;

/// Provider-independent boundary for native coding-agent detection.
pub trait IntegrationDetectionService {
    fn detect(&self) -> IntegrationDetection;
}

#[cfg(test)]
mod tests;
