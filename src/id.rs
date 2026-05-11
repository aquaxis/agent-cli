use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::error::{AppError, Result};

/// Unique identifier for an agent running as a single process.
///
/// Format is `agent-<ULID>`, automatically assigned at process startup. Also used
/// as the name in the registry (`<registry_dir>/<agent-id>.{sock,json}`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    /// Generate a new unique `AgentId`.
    pub fn new() -> Self {
        AgentId(format!("agent-{}", Ulid::new()))
    }

    /// Return the inner `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AgentId {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Err(AppError::config("agent id must not be empty"));
        }
        Ok(AgentId(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_ids() {
        let a = AgentId::new();
        let b = AgentId::new();
        assert_ne!(a, b);
        assert!(a.as_str().starts_with("agent-"));
    }

    #[test]
    fn parse_roundtrip() {
        let id = AgentId::new();
        let s = id.to_string();
        let parsed: AgentId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }
}
