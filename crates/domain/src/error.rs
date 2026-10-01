use std::fmt::{Display, Formatter};

pub type DomainResult<T> = Result<T, DomainError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    InvalidIdentifier {
        kind: &'static str,
    },
    InvalidTimestamp,
    InvalidTimeRange {
        entity: &'static str,
    },
    InvalidStatus {
        entity: &'static str,
        reason: &'static str,
    },
    InvalidReference {
        entity: &'static str,
        reason: &'static str,
    },
    InvalidValue {
        field: &'static str,
        reason: &'static str,
    },
    UnsupportedSchemaVersion {
        found: u16,
    },
    InvalidStatusTransition,
    Serialization(String),
}

impl Display for DomainError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidIdentifier { kind } => write!(formatter, "invalid {kind} identifier"),
            Self::InvalidTimestamp => formatter.write_str("invalid UTC timestamp"),
            Self::InvalidTimeRange { entity } => write!(formatter, "invalid {entity} time range"),
            Self::InvalidStatus { entity, reason } => {
                write!(formatter, "invalid {entity} status: {reason}")
            }
            Self::InvalidReference { entity, reason } => {
                write!(formatter, "invalid {entity} reference: {reason}")
            }
            Self::InvalidValue { field, reason } => write!(formatter, "invalid {field}: {reason}"),
            Self::UnsupportedSchemaVersion { found } => {
                write!(formatter, "unsupported domain schema version {found}")
            }
            Self::InvalidStatusTransition => {
                formatter.write_str("invalid session status transition")
            }
            Self::Serialization(message) => {
                write!(formatter, "domain serialization failed: {message}")
            }
        }
    }
}

impl std::error::Error for DomainError {}
