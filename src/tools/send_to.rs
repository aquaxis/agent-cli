use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::ipc::{client, registry, IpcMessage};
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct SendToTool;

#[derive(Debug, Deserialize)]
struct SendArgs {
    peer: String,
    text: String,
}

#[async_trait]
impl Tool for SendToTool {
    fn name(&self) -> &'static str {
        "send_to"
    }

    fn description(&self) -> &'static str {
        "Send a prompt to another agent (peer) running locally. Identify the peer by agent-id or display name."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "peer": {"type": "string"},
                "text": {"type": "string"}
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
        let msg = IpcMessage::Prompt {
            from: ctx.self_id.clone(),
            from_name: None,
            text: parsed.text.clone(),
        };
        match client::send(&peer.socket, &msg).await {
            Ok(IpcMessage::Ack { .. }) => {
                Ok(ToolOutput::ok(format!("delivered to {}", peer.id.as_str())))
            }
            Ok(IpcMessage::Error { message }) => Ok(ToolOutput::err(message)),
            Ok(other) => Ok(ToolOutput::err(format!("unexpected response: {:?}", other))),
            Err(e) => Ok(ToolOutput::err(e.to_string())),
        }
    }
}
