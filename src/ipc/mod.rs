use std::path::PathBuf;

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
        /// Optional reply socket path. When set, the receiving agent sends a
        /// `PromptReply` to this socket after processing the prompt.
        #[serde(default)]
        reply_to: Option<PathBuf>,
    },
    /// Reply carrying the assistant's response text back to the sender.
    PromptReply {
        /// Sender AgentId (the responding agent).
        from: AgentId,
        /// Assistant response text.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_with_reply_to_serializes() {
        let msg = IpcMessage::Prompt {
            from: AgentId::new(),
            from_name: Some("tester".into()),
            text: "hello".into(),
            reply_to: Some(PathBuf::from("/tmp/reply.sock")),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains("\"reply_to\""),
            "serialized JSON should contain reply_to: {json}"
        );
    }

    #[test]
    fn prompt_without_reply_to_defaults_none() {
        let json = r#"{"kind":"prompt","from":"abc","from_name":"tester","text":"hi"}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        match msg {
            IpcMessage::Prompt { reply_to, .. } => {
                assert!(reply_to.is_none(), "reply_to should default to None");
            }
            other => panic!("expected Prompt, got {:?}", other),
        }
    }

    #[test]
    fn prompt_reply_serializes() {
        let msg = IpcMessage::PromptReply {
            from: AgentId::new(),
            text: "response text".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            json.contains("\"kind\":\"prompt_reply\""),
            "serialized JSON should contain kind:prompt_reply: {json}"
        );
    }

    #[test]
    fn prompt_reply_roundtrip() {
        let msg = IpcMessage::PromptReply {
            from: AgentId::new(),
            text: "roundtrip text".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: IpcMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            IpcMessage::PromptReply { from, text } => {
                assert_eq!(text, "roundtrip text");
                // from is an AgentId — we can't compare directly, but verify it exists
                let _ = from;
            }
            other => panic!("expected PromptReply, got {:?}", other),
        }
    }
}
