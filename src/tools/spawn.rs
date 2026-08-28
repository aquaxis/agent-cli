use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::cli::RunArgs;
use crate::error::Result;
use crate::ipc::{client, IpcMessage};
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// LLM-callable tool that creates a detached agent-cli peer. The new agent runs
/// headless in its own session (it does not depend on this process), shares this
/// agent's config file (hence the same registry_dir), and is reachable
/// afterwards with the `send_to` tool. Opt-in: enable it in `[tools] enabled`.
pub struct SpawnTool;

#[derive(Debug, Deserialize)]
struct SpawnArgs {
    /// Display name for the new agent (optional).
    #[serde(default)]
    name: Option<String>,
    /// Backend override for the new agent (optional).
    #[serde(default)]
    provider: Option<String>,
    /// Model override for the new agent (optional).
    #[serde(default)]
    model: Option<String>,
    /// Persona file path for the new agent (optional).
    #[serde(default)]
    persona: Option<String>,
    /// Group id for the new agent (optional). When omitted, the new agent
    /// inherits this agent's group.
    #[serde(default)]
    group: Option<String>,
    /// If set, an initial prompt delivered to the new agent once it is running
    /// (fire-and-forget). Use `send_to` for a reply.
    #[serde(default)]
    prompt: Option<String>,
}

#[async_trait]
impl Tool for SpawnTool {
    fn name(&self) -> &str {
        "spawn"
    }

    fn description(&self) -> &str {
        "Create a new detached agent-cli peer that runs headless in its own session and does not depend on this process. It shares this agent's config (same registry), so you can reach it afterwards with send_to. Optionally give it a name/provider/model/persona and an initial prompt."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Display name for the new agent."},
                "provider": {"type": "string", "description": "Backend override (claude / claude-code / codex / ollama / opencode / opencode-go / llama.cpp)."},
                "model": {"type": "string", "description": "Model override."},
                "persona": {"type": "string", "description": "Persona file path."},
                "group": {"type": "string", "description": "Group id for the new agent. Omit to inherit this agent's group."},
                "prompt": {"type": "string", "description": "Optional initial prompt delivered to the new agent (fire-and-forget)."}
            }
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: SpawnArgs = serde_json::from_value(args)?;
        let run_args = RunArgs {
            name: parsed.name,
            // Explicit group wins; otherwise inherit this agent's group.
            group: parsed
                .group
                .or_else(|| ctx.group.as_ref().map(|g| g.to_string())),
            provider: parsed.provider,
            model: parsed.model,
            persona: parsed.persona.map(std::path::PathBuf::from),
            auto_approve_tools: false,
        };

        let entry = match crate::commands::spawn_detached(
            &ctx.config_source.path,
            &ctx.registry_dir,
            &run_args,
        )
        .await
        {
            Ok(e) => e,
            Err(e) => return Ok(ToolOutput::err(format!("spawn failed: {e}"))),
        };

        let base = format!(
            "spawned detached agent id={} name={} provider={} model={} socket={}",
            entry.id,
            entry.name.as_deref().unwrap_or("-"),
            entry.provider,
            entry.model,
            entry.socket.display()
        );

        if let Some(text) = parsed.prompt {
            let msg = IpcMessage::Prompt {
                from: ctx.self_id.clone(),
                from_name: None,
                text,
                reply_to: None,
            };
            match client::send(&entry.socket, &msg).await {
                Ok(IpcMessage::Ack { .. }) => {
                    Ok(ToolOutput::ok(format!("{base}; initial prompt delivered")))
                }
                Ok(other) => Ok(ToolOutput::ok(format!(
                    "{base}; initial prompt not acked: {other:?}"
                ))),
                Err(e) => Ok(ToolOutput::ok(format!(
                    "{base}; initial prompt failed: {e}"
                ))),
            }
        } else {
            Ok(ToolOutput::ok(base))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_tool_metadata() {
        let t = SpawnTool;
        assert_eq!(t.name(), "spawn");
        let schema = t.schema();
        assert_eq!(schema["type"], "object");
        // All fields optional: no "required" array.
        assert!(schema.get("required").is_none());
        assert!(schema["properties"].get("name").is_some());
        assert!(schema["properties"].get("group").is_some());
        assert!(schema["properties"].get("prompt").is_some());
    }

    #[test]
    fn spawn_args_all_optional() {
        // An empty object deserializes (every field defaults to None).
        let parsed: SpawnArgs = serde_json::from_value(json!({})).unwrap();
        assert!(parsed.name.is_none());
        assert!(parsed.provider.is_none());
        assert!(parsed.group.is_none());
        assert!(parsed.prompt.is_none());
    }

    #[test]
    fn spawn_args_parses_group() {
        let parsed: SpawnArgs = serde_json::from_value(json!({"group": "team"})).unwrap();
        assert_eq!(parsed.group.as_deref(), Some("team"));
    }
}
