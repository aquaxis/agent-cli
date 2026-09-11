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
    /// Text to add to the receiving agent's conversation **without starting a
    /// turn**: a result a peer is reporting, which expects no answer. The
    /// receiver appends it and goes on waiting, so a fan-out of children
    /// reporting back costs the parent no provider calls.
    ///
    /// Field names match `Prompt`'s, so the two read alike on the wire and in
    /// the history they produce.
    Context {
        /// Sender AgentId.
        from: AgentId,
        /// Sender display name (optional).
        from_name: Option<String>,
        /// Message body.
        text: String,
    },
    /// Request the receiving agent to shut down gracefully. The receiver Acks
    /// this and then converges on its normal shutdown/cleanup sequence. Used to
    /// stop a headless (detached `serve`) agent that has no controlling TTY.
    Shutdown,
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
    fn shutdown_roundtrips() {
        let json = serde_json::to_string(&IpcMessage::Shutdown).unwrap();
        assert_eq!(json, r#"{"kind":"shutdown"}"#);
        let back: IpcMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, IpcMessage::Shutdown));
    }

    /// The context delivery's wire shape, and the guarantee the fallback rests
    /// on: an agent that does not know this kind answers with a parse error
    /// rather than failing, so the sender can resend as a prompt.
    #[test]
    fn context_serializes_and_roundtrips() {
        let msg = IpcMessage::Context {
            from: AgentId::new(),
            from_name: Some("reviewer".into()),
            text: "the result".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""kind":"context""#), "{json}");
        match serde_json::from_str::<IpcMessage>(&json).unwrap() {
            IpcMessage::Context {
                from_name, text, ..
            } => {
                assert_eq!(from_name.as_deref(), Some("reviewer"));
                assert_eq!(text, "the result");
            }
            other => panic!("expected Context, got {other:?}"),
        }
    }

    /// Adding a variant must not move any existing one: these are the bytes
    /// v0.16.0 wrote, and a peer of either version has to keep reading them.
    #[test]
    fn the_existing_variants_keep_their_wire_shape() {
        let id: AgentId = "agent-01ABC".parse().unwrap();
        let cases = [
            (
                IpcMessage::Prompt {
                    from: id.clone(),
                    from_name: Some("a".into()),
                    text: "t".into(),
                    reply_to: None,
                },
                r#"{"kind":"prompt","from":"agent-01ABC","from_name":"a","text":"t","reply_to":null}"#,
            ),
            (
                IpcMessage::PromptReply {
                    from: id.clone(),
                    text: "r".into(),
                },
                r#"{"kind":"prompt_reply","from":"agent-01ABC","text":"r"}"#,
            ),
            (IpcMessage::Ack { id: 0 }, r#"{"kind":"ack","id":0}"#),
            (
                IpcMessage::Error {
                    message: "m".into(),
                },
                r#"{"kind":"error","message":"m"}"#,
            ),
            (IpcMessage::Ping, r#"{"kind":"ping"}"#),
            (IpcMessage::Pong, r#"{"kind":"pong"}"#),
            (IpcMessage::Shutdown, r#"{"kind":"shutdown"}"#),
        ];
        for (msg, expected) in cases {
            assert_eq!(serde_json::to_string(&msg).unwrap(), expected);
        }
    }

    #[test]
    fn an_unknown_kind_is_a_parse_error_not_a_panic() {
        let err = serde_json::from_str::<IpcMessage>(r#"{"kind":"from_the_future","text":"x"}"#);
        assert!(
            err.is_err(),
            "an unknown kind must fail to parse so the receiver can answer with an error"
        );
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
