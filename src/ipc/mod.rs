use serde::{Deserialize, Serialize};

use crate::id::AgentId;

pub mod client;
pub mod registry;
pub mod server;

/// JSON Lines message used for inter-process communication.
///
/// Sent and received as one message per line over Unix domain sockets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcMessage {
    /// Prompt sent to another agent.
    Prompt {
        /// Sender AgentId.
        from: AgentId,
        /// Sender display name (optional).
        from_name: Option<String>,
        /// Message body.
        text: String,
    },
    /// Acknowledgment of successful receipt.
    Ack {
        /// Message identifier (reserved for future use, currently always 0).
        id: u64,
    },
    /// Error response.
    Error {
        /// Human-readable error message.
        message: String,
    },
    /// Connectivity check (request).
    Ping,
    /// Connectivity check (response).
    Pong,
}
