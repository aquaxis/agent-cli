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
    #[serde(default)]
    wait_reply: bool,
}

#[async_trait]
impl Tool for SendToTool {
    fn name(&self) -> &'static str {
        "send_to"
    }

    fn description(&self) -> &'static str {
        "Send a prompt to another agent (peer) running locally. Identify the peer by agent-id or display name. Set wait_reply=true to wait for the peer's response."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "peer": {"type": "string"},
                "text": {"type": "string"},
                "wait_reply": {
                    "type": "boolean",
                    "default": false,
                    "description": "If true, wait for the peer's AI response and return it as the tool output. Default: false (fire-and-forget)."
                }
            },
            "required": ["peer", "text"]
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: SendArgs = serde_json::from_value(args)?;
        let peer = match registry::resolve_peer(&ctx.registry_dir, &parsed.peer) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(e.to_string())),
        };

        if !parsed.wait_reply {
            // Fire-and-forget (existing behavior)
            let msg = IpcMessage::Prompt {
                from: ctx.self_id.clone(),
                from_name: None,
                text: parsed.text.clone(),
                reply_to: None,
            };
            match client::send(&peer.socket, &msg).await {
                Ok(IpcMessage::Ack { .. }) => {
                    Ok(ToolOutput::ok(format!("delivered to {}", peer.id.as_str())))
                }
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
            registry_dir: registry_dir.clone(),
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
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
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
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
        };
        write_registry_entry(registry_dir, &entry);
        entry
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
