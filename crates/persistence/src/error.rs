use rusqlite::{Error as SqliteError, ErrorCode};

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    Open,
    Permission,
    Migration,
    SchemaTooNew,
    ChecksumMismatch,
    Corrupt,
    Busy,
    Constraint,
    Serialization,
    DomainValidation,
    PolicyRejected,
    InvalidInput,
    Acknowledgement,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Open => "local storage could not be opened",
            Self::Permission => "local storage permissions could not be secured",
            Self::Migration => "local storage migration failed",
            Self::SchemaTooNew => "local storage was created by a newer Mochi version",
            Self::ChecksumMismatch => "local storage migration history is inconsistent",
            Self::Corrupt => "local storage failed integrity validation",
            Self::Busy => "local storage is busy",
            Self::Constraint => "local storage rejected inconsistent data",
            Self::Serialization => "local data could not be encoded or decoded",
            Self::DomainValidation => "persisted domain data is invalid",
            Self::PolicyRejected => "capture policy rejected the evidence",
            Self::InvalidInput => "the storage request is invalid",
            Self::Acknowledgement => "durable evidence could not be acknowledged",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for StorageError {}

pub(crate) fn map_sqlite(error: SqliteError) -> StorageError {
    match error {
        SqliteError::SqliteFailure(inner, _) => match inner.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => StorageError::Busy,
            ErrorCode::ConstraintViolation => StorageError::Constraint,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => StorageError::Corrupt,
            _ => StorageError::Open,
        },
        SqliteError::FromSqlConversionFailure(..)
        | SqliteError::IntegralValueOutOfRange(..)
        | SqliteError::InvalidColumnType(..) => StorageError::Serialization,
        _ => StorageError::Open,
    }
}
