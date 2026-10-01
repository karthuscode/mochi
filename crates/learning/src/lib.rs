//! Bounded, provider-independent learning contracts. Generated content is never executable.
mod bundle;
mod contract;
mod question;
mod schema;
pub use bundle::*;
pub use contract::*;
pub use question::*;
pub use schema::{ENDPOINT, MODEL, generation_request, grading_request, response_text};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LearningError {
    InvalidInput,
    TooLarge,
    Sanitization,
    InsufficientContext,
    InvalidOutput,
    InvalidReference,
    InvalidQuestion,
    Refused,
    Incomplete,
}
pub type LearningResult<T> = Result<T, LearningError>;
impl std::fmt::Display for LearningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "learning input is invalid",
            Self::TooLarge => "learning content exceeds safe bounds",
            Self::Sanitization => "learning content could not be sanitized",
            Self::InsufficientContext => "there is insufficient technical context",
            Self::InvalidOutput => "generated learning content is invalid",
            Self::InvalidReference => "generated evidence references are invalid",
            Self::InvalidQuestion => "the question is not safely gradeable",
            Self::Refused => "the provider refused this request",
            Self::Incomplete => "the provider returned an incomplete result",
        })
    }
}
impl std::error::Error for LearningError {}
#[cfg(test)]
mod tests;
