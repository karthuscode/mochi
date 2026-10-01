use mochi_persistence::StorageError;

pub type AssemblyResult<T> = Result<T, AssemblyError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssemblyError {
    InvalidEvidenceRelationship,
    UnsupportedIngressSchema,
    DomainValidation,
    AmbiguousAssociation,
    BoundsExceeded,
    Storage(StorageError),
}

impl std::fmt::Display for AssemblyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::InvalidEvidenceRelationship => "evidence relationships are invalid",
            Self::UnsupportedIngressSchema => "evidence schema is unsupported",
            Self::DomainValidation => "assembled session failed domain validation",
            Self::AmbiguousAssociation => "evidence association is ambiguous",
            Self::BoundsExceeded => "assembly input exceeds configured bounds",
            Self::Storage(error) => return error.fmt(formatter),
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for AssemblyError {}

impl From<StorageError> for AssemblyError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}
