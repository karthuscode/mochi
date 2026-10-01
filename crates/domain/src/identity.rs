use crate::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! domain_id {
    ($name:ident, $label:literal) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn parse(value: &str) -> DomainResult<Self> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| DomainError::InvalidIdentifier { kind: $label })
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

domain_id!(ProjectId, "project");
domain_id!(SessionId, "session");
domain_id!(TurnId, "turn");
domain_id!(EventId, "event");
domain_id!(ToolExecutionId, "tool execution");
domain_id!(CommandExecutionId, "command execution");
domain_id!(FileChangeId, "file change");
