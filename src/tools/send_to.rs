use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::ipc::server::IpcServer;
use crate::ipc::{client, registry, IpcMessage};
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct SendToTool;

#[derive(Debug, Deserialize)]
struct SendArgs {
    peer: String,
    text: String,
    /// `"prompt"` (default) | `"report"` | `"ask"`. See [`Delivery`].
    #[serde(default)]
    delivery: Option<String>,
    /// Deprecated alias: `true` means `delivery = "ask"`. Kept so calls and
    /// personas written before `delivery` existed keep working.
    #[serde(default)]
    wait_reply: bool,
}

/// How a message is delivered — and, more to the point, what it costs the peer.
///
/// Only the sender knows whether it is asking something or handing over a
/// result, so the sender chooses: a report that arrives as a prompt makes the
/// peer run a turn and compose an answer nobody will read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivery {
    /// A prompt the peer answers in its own session; nothing comes back here.
    Prompt,
    /// A result added to the peer's conversation without running a turn.
    Report,
    /// A prompt whose answer is waited for and returned as the tool output.
    Ask,
}

impl Delivery {
    /// `delivery` wins when given; otherwise the deprecated `wait_reply` flag;
    /// otherwise a plain prompt, which is what this tool has always done.
    fn resolve(delivery: Option<&str>, wait_reply: bool) -> std::result::Result<Self, String> {
        match delivery.map(str::trim) {
            None | Some("") => Ok(if wait_reply {
                Delivery::Ask
            } else {
                Delivery::Prompt
            }),
            Some("prompt") => Ok(Delivery::Prompt),
            Some("report") => Ok(Delivery::Report),
            Some("ask") => Ok(Delivery::Ask),
            Some(other) => Err(format!(
                "unknown delivery {other:?}: use \"prompt\" (the peer answers in its own \
                 session), \"report\" (added to the peer's conversation without costing it a \
                 turn) or \"ask\" (wait for the peer's answer)"
            )),
        }
    }
}

#[async_trait]
impl Tool for SendToTool {
    fn name(&self) -> &str {
        "send_to"
    }

    fn description(&self) -> &str {
        "Send a message to another agent (peer) running locally, by agent-id or display name. Choose how it is delivered: delivery=\"prompt\" (default) has the peer answer it in its own session; delivery=\"report\" adds it to the peer's conversation without making it run a turn, which is how a child hands a finished result back to the agent that created it; delivery=\"ask\" waits for the peer's answer and returns it here."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "peer": {"type": "string"},
                "text": {"type": "string"},
                "delivery": {
                    "type": "string",
                    "enum": ["prompt", "report", "ask"],
                    "default": "prompt",
                    "description": "prompt = the peer answers it in its own session (nothing returns here); report = added to the peer's conversation without costing it a turn, for handing over a finished result; ask = wait for the peer's answer and return it here."
                },
                "wait_reply": {
                    "type": "boolean",
                    "default": false,
                    "description": "Deprecated alias for delivery=\"ask\"."
                }
            },
            "required": ["peer", "text"]
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: SendArgs = serde_json::from_value(args)?;
        let delivery = match Delivery::resolve(parsed.delivery.as_deref(), parsed.wait_reply) {
            Ok(d) => d,
            Err(why) => return Ok(ToolOutput::err(why)),
        };
        let peer = match registry::resolve_peer(&ctx.registry_dir, &parsed.peer) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(e.to_string())),
        };

        if delivery == Delivery::Report {
            let msg = IpcMessage::Context {
                from: ctx.self_id.clone(),
                from_name: None,
                text: parsed.text.clone(),
            };
            match client::send(&peer.socket, &msg).await {
                Ok(IpcMessage::Ack { .. }) => {
                    return Ok(ToolOutput::ok(format!(
                        "reported to {} (added to its conversation; it runs no turn and sends no answer)",
                        peer.id.as_str()
                    )))
                }
                // An agent-cli that predates this delivery cannot parse the
                // message and says so. The text still has to arrive, so send it
                // as an ordinary prompt — and say that the peer will answer it
                // rather than only record it, which changes what to do next.
                Ok(IpcMessage::Error { message }) => {
                    let fallback = IpcMessage::Prompt {
                        from: ctx.self_id.clone(),
                        from_name: None,
                        text: parsed.text.clone(),
                        reply_to: None,
                    };
                    return match client::send(&peer.socket, &fallback).await {
                        Ok(IpcMessage::Ack { .. }) => Ok(ToolOutput::ok(format!(
                            "delivered to {} as a prompt: that agent does not support report \
                             delivery, so it will answer this in its own session instead of only \
                             recording it (it said: {message})",
                            peer.id.as_str()
                        ))),
                        Ok(IpcMessage::Error { message }) => Ok(ToolOutput::err(message)),
                        Ok(other) => {
                            Ok(ToolOutput::err(format!("unexpected response: {:?}", other)))
                        }
                        Err(e) => Ok(ToolOutput::err(e.to_string())),
                    };
                }
                Ok(other) => return Ok(ToolOutput::err(format!("unexpected response: {:?}", other))),
                Err(e) => return Ok(ToolOutput::err(e.to_string())),
            }
        }

        if delivery == Delivery::Prompt {
            // Fire-and-forget (existing behavior)
            let msg = IpcMessage::Prompt {
                from: ctx.self_id.clone(),
                from_name: None,
                text: parsed.text.clone(),
                reply_to: None,
            };
            match client::send(&peer.socket, &msg).await {
                Ok(IpcMessage::Ack { .. }) => Ok(ToolOutput::ok(format!(
                    "delivered to {} as a prompt (it answers in its own session)",
                    peer.id.as_str()
                ))),
                Ok(IpcMessage::Error { message }) => Ok(ToolOutput::err(message)),
                Ok(other) => Ok(ToolOutput::err(format!("unexpected response: {:?}", other))),
                Err(e) => Ok(ToolOutput::err(e.to_string())),
            }
        } else {
            // Wait for reply
            let reply_dir = match tempfile::tempdir() {
                Ok(d) => d,
                Err(e) => return Ok(ToolOutput::err(format!("temp dir: {e}"))),
            };
            let reply_socket = reply_dir.path().join("reply.sock");
            let mut reply_server = match IpcServer::bind(reply_socket.clone()).await {
                Ok(s) => s,
                Err(e) => return Ok(ToolOutput::err(format!("bind reply socket: {e}"))),
            };
            let mut reply_rx = reply_server.take_rx().expect("rx available after bind");

            let msg = IpcMessage::Prompt {
                from: ctx.self_id.clone(),
                from_name: None,
                text: parsed.text.clone(),
                reply_to: Some(reply_socket.clone()),
            };
            match client::send(&peer.socket, &msg).await {
                Ok(IpcMessage::Ack { .. }) => {}
                Ok(IpcMessage::Error { message }) => return Ok(ToolOutput::err(message)),
                Ok(other) => {
                    return Ok(ToolOutput::err(format!("unexpected response: {:?}", other)))
                }
                Err(e) => return Ok(ToolOutput::err(e.to_string())),
            }

            let result = tokio::time::timeout(
                Duration::from_secs(120),
                reply_rx.recv(),
            )
            .await;

            drop(reply_server);

            match result {
                Ok(Some(IpcMessage::PromptReply { text, .. })) => Ok(ToolOutput::ok(text)),
                Ok(Some(other)) => {
                    Ok(ToolOutput::err(format!("unexpected reply: {:?}", other)))
                }
                Ok(None) => Ok(ToolOutput::err("reply channel closed without response")),
                Err(_) => Ok(ToolOutput::err("timed out waiting for reply after 120s")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::server::IpcServer;
    use crate::ipc::IpcMessage;
    use crate::tools::ToolCtx;
    use crate::id::AgentId;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn build_ctx(registry_dir: &PathBuf) -> ToolCtx {
        ToolCtx {
            self_id: AgentId::new(),
            group: None,
            registry_dir: registry_dir.clone(),
            config_source: Default::default(),
            event_tx: None,
            ancestors: Vec::new(),
            spawn_limits: crate::swarm::SpawnLimits::default(),
        }
    }

    /// Write a registry entry JSON file manually (without RegistryHandle, so it
    /// is not cleaned up by Drop when the setup function returns).
    fn write_registry_entry(dir: &PathBuf, entry: &crate::ipc::registry::RegistryEntry) {
        let meta_path = dir.join(format!("{}.json", entry.id.as_str()));
        let raw = serde_json::to_vec_pretty(entry).unwrap();
        std::fs::write(&meta_path, raw).unwrap();
    }

    /// Create a mock peer: bind an IpcServer, register a registry entry,
    /// spawn a task that receives a Prompt and sends a PromptReply to the
    /// specified reply_to socket.
    async fn setup_mock_peer_with_reply(
        registry_dir: &PathBuf,
        name: &str,
        reply_text: &str,
    ) -> crate::ipc::registry::RegistryEntry {
        use crate::ipc::registry::RegistryEntry;
        use chrono::Utc;

        let id = AgentId::new();
        let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));
        let mut server = IpcServer::bind(socket_path.clone()).await.unwrap();
        let mut rx = server.take_rx().unwrap();

        let reply_text = reply_text.to_string();
        tokio::spawn(async move {
            let _server = server;
            if let Some(msg) = rx.recv().await {
                if let IpcMessage::Prompt {
                    reply_to: Some(reply_socket),
                    ..
                } = msg
                {
                    let reply = IpcMessage::PromptReply {
                        from: AgentId::new(),
                        text: reply_text,
                    };
                    let _ = client::send(&reply_socket, &reply).await;
                }
            }
        });

        let entry = RegistryEntry {
            id: id.clone(),
            name: Some(name.into()),
            group: None,
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
            ancestors: Vec::new(),
        };
        write_registry_entry(registry_dir, &entry);
        entry
    }

    /// Create a mock peer that does NOT send a reply (for fire-and-forget test).
    async fn setup_mock_peer_no_reply(
        registry_dir: &PathBuf,
        name: &str,
    ) -> crate::ipc::registry::RegistryEntry {
        use crate::ipc::registry::RegistryEntry;
        use chrono::Utc;

        let id = AgentId::new();
        let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));
        let mut server = IpcServer::bind(socket_path.clone()).await.unwrap();
        let mut rx = server.take_rx().unwrap();

        tokio::spawn(async move {
            let _server = server;
            let _ = rx.recv().await;
        });

        let entry = RegistryEntry {
            id: id.clone(),
            name: Some(name.into()),
            group: None,
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
            ancestors: Vec::new(),
        };
        write_registry_entry(registry_dir, &entry);
        entry
    }

    #[test]
    fn the_delivery_is_chosen_by_the_sender_with_the_old_flag_still_working() {
        assert_eq!(Delivery::resolve(None, false).unwrap(), Delivery::Prompt);
        // The deprecated flag keeps its meaning for calls written before
        // `delivery` existed.
        assert_eq!(Delivery::resolve(None, true).unwrap(), Delivery::Ask);
        assert_eq!(Delivery::resolve(Some("prompt"), true).unwrap(), Delivery::Prompt);
        assert_eq!(Delivery::resolve(Some("report"), false).unwrap(), Delivery::Report);
        assert_eq!(Delivery::resolve(Some("ask"), false).unwrap(), Delivery::Ask);
        assert_eq!(Delivery::resolve(Some(" report "), false).unwrap(), Delivery::Report);
        assert_eq!(Delivery::resolve(Some(""), true).unwrap(), Delivery::Ask);
        // Guessing would be wrong: the difference is whether the peer spends a
        // turn, so an unknown value names the three valid ones.
        let err = Delivery::resolve(Some("shout"), false).unwrap_err();
        assert!(err.contains("\"shout\""), "{err}");
        for valid in ["prompt", "report", "ask"] {
            assert!(err.contains(valid), "{err}");
        }
    }

    /// A peer that speaks this version records the report and runs no turn.
    #[tokio::test]
    async fn report_delivery_sends_a_context_message() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let (entry, mut rx) = setup_listening_peer(&dir, "worker").await;
        write_registry_entry(&dir, &entry);

        let out = SendToTool
            .invoke(
                json!({"peer": "worker", "text": "the finished result", "delivery": "report"}),
                &build_ctx(&dir),
            )
            .await
            .unwrap();
        assert!(out.ok, "{}", out.content);
        assert!(out.content.contains("runs no turn"), "{}", out.content);

        match rx.recv().await.expect("the peer received something") {
            IpcMessage::Context { text, .. } => assert_eq!(text, "the finished result"),
            other => panic!("a report must not arrive as a prompt: {other:?}"),
        }
    }

    #[tokio::test]
    async fn prompt_delivery_still_sends_a_prompt() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let (entry, mut rx) = setup_listening_peer(&dir, "worker").await;
        write_registry_entry(&dir, &entry);

        let out = SendToTool
            .invoke(json!({"peer": "worker", "text": "do this"}), &build_ctx(&dir))
            .await
            .unwrap();
        assert!(out.ok, "{}", out.content);
        assert!(out.content.contains("as a prompt"), "{}", out.content);
        match rx.recv().await.expect("the peer received something") {
            IpcMessage::Prompt { text, reply_to, .. } => {
                assert_eq!(text, "do this");
                assert!(reply_to.is_none(), "the default is fire-and-forget");
            }
            other => panic!("expected a prompt, got {other:?}"),
        }
    }

    /// An older agent-cli cannot parse the context message and answers with an
    /// error. The text still has to arrive, so it goes as a prompt — and the
    /// result says the peer will answer it rather than only record it.
    #[tokio::test]
    async fn an_older_peer_falls_back_to_a_prompt() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        let (entry, socket) = registry_entry_for(&dir, "legacy");
        write_registry_entry(&dir, &entry);

        // Stand in for a v0.16.0 peer: answer anything it cannot parse with the
        // same error the real server sends, and accept a prompt.
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let seen_task = seen.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            while let Ok((stream, _)) = listener.accept().await {
                let seen = seen_task.clone();
                tokio::spawn(async move {
                    let (read_half, mut write_half) = stream.into_split();
                    let mut lines = BufReader::new(read_half).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let response = if line.contains(r#""kind":"context""#) {
                            seen.lock().unwrap().push("context".into());
                            r#"{"kind":"error","message":"parse error: unknown variant `context`"}"#
                                .to_string()
                        } else {
                            seen.lock().unwrap().push("prompt".into());
                            r#"{"kind":"ack","id":0}"#.to_string()
                        };
                        let _ = write_half.write_all(response.as_bytes()).await;
                        let _ = write_half.write_all(b"\n").await;
                    }
                });
            }
        });

        let out = SendToTool
            .invoke(
                json!({"peer": "legacy", "text": "the result", "delivery": "report"}),
                &build_ctx(&dir),
            )
            .await
            .unwrap();
        assert!(out.ok, "the sender's turn must not fail: {}", out.content);
        assert!(out.content.contains("as a prompt"), "{}", out.content);
        assert!(
            out.content.contains("does not support report delivery"),
            "the model is told the peer will answer instead: {}",
            out.content
        );
        assert_eq!(
            *seen.lock().unwrap(),
            vec!["context".to_string(), "prompt".to_string()],
            "the context was tried first, then the prompt"
        );
    }

    /// A registry entry and its socket path, without a server behind it.
    fn registry_entry_for(
        dir: &std::path::Path,
        name: &str,
    ) -> (crate::ipc::registry::RegistryEntry, PathBuf) {
        use crate::ipc::registry::RegistryEntry;
        use chrono::Utc;
        let id = AgentId::new();
        let socket = dir.join(format!("{}.sock", id.as_str()));
        (
            RegistryEntry {
                id,
                name: Some(name.to_string()),
                group: None,
                pid: std::process::id(),
                started_at: Utc::now(),
                provider: "ollama".into(),
                model: "m".into(),
                socket: socket.clone(),
                persona: None,
                ancestors: Vec::new(),
            },
            socket,
        )
    }

    /// A peer that accepts messages and hands them to the caller.
    async fn setup_listening_peer(
        dir: &std::path::Path,
        name: &str,
    ) -> (
        crate::ipc::registry::RegistryEntry,
        tokio::sync::mpsc::Receiver<IpcMessage>,
    ) {
        let (entry, socket) = registry_entry_for(dir, name);
        let mut server = IpcServer::bind(socket).await.unwrap();
        let rx = server.take_rx().unwrap();
        // The server lives as long as the test does.
        std::mem::forget(server);
        (entry, rx)
    }

    #[tokio::test]
    async fn send_to_wait_reply_returns_response() {
        let dir = TempDir::new().unwrap();
        let registry_dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&registry_dir).unwrap();

        let _entry =
            setup_mock_peer_with_reply(&registry_dir, "mock-peer", "hello from peer").await;

        let tool = SendToTool;
        let ctx = build_ctx(&registry_dir);
        let args = json!({
            "peer": "mock-peer",
            "text": "hi",
            "wait_reply": true
        });

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            tool.invoke(args, &ctx),
        )
        .await
        .expect("invoke timeout");

        assert!(result.is_ok(), "invoke should succeed");
        let output = result.unwrap();
        assert!(output.ok, "output should be ok, got: {}", output.content);
        assert_eq!(output.content, "hello from peer");
    }

    #[tokio::test]
    async fn send_to_without_wait_reply_is_fire_and_forget() {
        let dir = TempDir::new().unwrap();
        let registry_dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&registry_dir).unwrap();

        let entry = setup_mock_peer_no_reply(&registry_dir, "mock-peer2").await;

        let tool = SendToTool;
        let ctx = build_ctx(&registry_dir);
        let args = json!({
            "peer": "mock-peer2",
            "text": "fire and forget"
        });

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            tool.invoke(args, &ctx),
        )
        .await
        .expect("invoke timeout");

        assert!(result.is_ok(), "invoke should succeed");
        let output = result.unwrap();
        assert!(output.ok, "output should be ok, got: {}", output.content);
        assert!(
            output.content.contains(&entry.id.to_string()),
            "output should contain peer id, got: {}",
            output.content
        );
        assert!(
            output.content.starts_with("delivered to"),
            "output should start with 'delivered to', got: {}",
            output.content
        );
    }
}
