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
        "Create a new detached agent-cli peer that runs headless in its own session and does not depend on this process. It shares this agent's config (same registry), so you can reach it afterwards with send_to, see it with list_agents and stop it with stop_agent. The number of live children and the depth of the chain are limited; when a limit is reached this returns an error saying which. Optionally give it a name/provider/model/persona and an initial prompt."
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

        // The bound comes first, so a refusal costs nothing and creates
        // nothing. Live children are counted from the registry, which prunes
        // dead peers as it reads — a child that exited has already freed its
        // slot.
        let live = crate::ipc::registry::children_of(&ctx.registry_dir, &ctx.self_id)
            .map(|c| c.len() as u32)
            .unwrap_or(0);
        if let crate::swarm::Allowance::Denied(why) =
            crate::swarm::spawn_allowance(live, ctx.ancestors.len() as u32, ctx.spawn_limits)
        {
            return Ok(ToolOutput::err(why));
        }

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

        let mut chain = ctx.ancestors.clone();
        chain.push(ctx.self_id.clone());
        let entry = match crate::commands::spawn_detached(
            &ctx.config_source.path,
            &ctx.registry_dir,
            &run_args,
            // The child's chain is this agent's chain plus this agent itself.
            &chain,
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
    use crate::id::AgentId;

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

    /// At a limit the tool refuses with the reason and creates nothing. The
    /// registry directory does not exist, so any attempt to spawn would be
    /// visible: a denial must happen before that.
    #[tokio::test]
    async fn the_tool_refuses_at_each_limit_without_creating_anything() {
        use crate::swarm::SpawnLimits;
        use crate::tools::list_agents::tests_support::{ctx, register};
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let _c1 = register(dir, "c1", std::slice::from_ref(&me)).await;
        let _c2 = register(dir, "c2", std::slice::from_ref(&me)).await;

        // Two live children against a limit of two.
        let mut at_child_limit = ctx(dir, me.clone(), &[]);
        at_child_limit.spawn_limits = SpawnLimits {
            max_children: 2,
            max_depth: 4,
        };
        let out = SpawnTool.invoke(json!({}), &at_child_limit).await.unwrap();
        assert!(!out.ok, "{}", out.content);
        assert!(out.content.contains("child limit reached"), "{}", out.content);
        assert!(out.content.contains("2 live child agent(s)"), "{}", out.content);

        // An agent as deep as the chain is allowed to go.
        let ancestors = vec![AgentId::new(), AgentId::new()];
        let mut at_depth_limit = ctx(dir, AgentId::new(), &ancestors);
        at_depth_limit.spawn_limits = SpawnLimits {
            max_children: 4,
            max_depth: 2,
        };
        let out = SpawnTool.invoke(json!({}), &at_depth_limit).await.unwrap();
        assert!(!out.ok, "{}", out.content);
        assert!(out.content.contains("depth limit reached"), "{}", out.content);

        // Nothing new was registered by either refusal.
        let live = crate::ipc::registry::list_entries(dir).unwrap();
        assert_eq!(live.len(), 3, "no agent may be created by a refused call");
    }

    #[test]
    fn spawn_args_parses_group() {
        let parsed: SpawnArgs = serde_json::from_value(json!({"group": "team"})).unwrap();
        assert_eq!(parsed.group.as_deref(), Some("team"));
    }
}
