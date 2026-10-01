use crate::{DomainError, DomainResult};
use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct UtcTimestamp(String);

impl UtcTimestamp {
    pub fn parse(value: &str) -> DomainResult<Self> {
        let parsed =
            OffsetDateTime::parse(value, &Rfc3339).map_err(|_| DomainError::InvalidTimestamp)?;
        if parsed.offset() != UtcOffset::UTC {
            return Err(DomainError::InvalidTimestamp);
        }
        let canonical = parsed
            .format(&Rfc3339)
            .map_err(|_| DomainError::InvalidTimestamp)?;
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for UtcTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}
