use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::ipc::registry;
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// LLM-callable tool that stops an agent this agent created.
///
/// It may only reach the caller's **own subtree**: a sibling, the agent that
/// created the caller, the caller itself, and any unrelated peer are refused.
/// That is what makes stopping safe to offer by default — an agent can undo
/// what it did and nothing more. A person keeps the unrestricted
/// `agent-cli stop` and the REPL's `/stop`.
pub struct StopAgentTool;

#[derive(Debug, Deserialize)]
struct StopArgs {
    /// Agent id or display name.
    agent: String,
}

#[async_trait]
impl Tool for StopAgentTool {
    fn name(&self) -> &str {
        "stop_agent"
    }

    fn description(&self) -> &str {
        "Stop an agent you created, by id or name, shutting it down gracefully. You can only stop agents in your own tree (the ones you spawned, and the ones they spawned); anything else is refused. Use list_agents to see what you can stop."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {"type": "string", "description": "Agent id or display name, as shown by list_agents."}
            },
            "required": ["agent"]
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: StopArgs = serde_json::from_value(args)?;
        let key = parsed.agent.trim();

        let target = match registry::resolve_peer(&ctx.registry_dir, key) {
            Ok(t) => t,
            Err(_) => {
                return Ok(ToolOutput::err(format!(
                    "no running agent matches {key:?}. Use list_agents to see the agents you can stop."
                )))
            }
        };

        if let Err(why) = stop_permission(&target, &ctx.self_id, &ctx.ancestors, key) {
            return Ok(ToolOutput::err(why));
        }

        match crate::commands::stop_peer(&ctx.registry_dir, key).await {
            Ok(()) => Ok(ToolOutput::ok(format!(
                "stopped {} (name={})",
                target.id,
                target.name.as_deref().unwrap_or("-")
            ))),
            Err(e) => Ok(ToolOutput::err(format!("could not stop {key:?}: {e}"))),
        }
    }
}

/// Whether this agent may stop `target`, and why not when it may not.
///
/// Pure, so the rule that decides what an agent can reach is a table in a test
/// rather than a branch that has to be exercised against real processes. The
/// refusal names the relationship: the model can tell "not mine" from "does not
/// exist" and act accordingly.
fn stop_permission(
    target: &registry::RegistryEntry,
    me: &crate::id::AgentId,
    my_ancestors: &[crate::id::AgentId],
    key: &str,
) -> std::result::Result<(), String> {
    if target.id.as_str() == me.as_str() {
        return Err(
            "that is this agent itself; stopping yourself is not something this tool does."
                .to_string(),
        );
    }
    if target.is_descendant_of(me) {
        return Ok(());
    }
    let relation = if my_ancestors
        .iter()
        .any(|a| a.as_str() == target.id.as_str())
    {
        "it is the agent that created you"
    } else {
        "you did not create it"
    };
    Err(format!(
        "{key:?} is not in your tree: {relation}. You can only stop agents you spawned; \
         list_agents shows them."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::AgentId;
    use crate::ipc::registry::RegistryEntry;
    use crate::tools::list_agents::tests_support::{ctx, register};
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn an_agent_outside_the_tree_is_refused() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let other_root = register(dir, "other", &[]).await;
        let sibling = register(dir, "sibling", std::slice::from_ref(&other_root)).await;

        for (key, expect) in [("other", "you did not create it"), ("sibling", "you did not create it")] {
            let out = StopAgentTool
                .invoke(json!({ "agent": key }), &ctx(dir, me.clone(), &[]))
                .await
                .unwrap();
            assert!(!out.ok, "{key} must be refused: {}", out.content);
            assert!(out.content.contains(expect), "{}", out.content);
        }
        // Nothing was stopped: both are still registered.
        let live = registry::list_entries(dir).unwrap();
        assert!(live.iter().any(|e| e.id.as_str() == other_root.as_str()));
        assert!(live.iter().any(|e| e.id.as_str() == sibling.as_str()));
    }

    #[tokio::test]
    async fn the_agent_that_created_me_is_refused_by_name() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let parent = register(dir, "parent", &[]).await;
        let me = register(dir, "me", std::slice::from_ref(&parent)).await;

        let out = StopAgentTool
            .invoke(json!({"agent": "parent"}), &ctx(dir, me, std::slice::from_ref(&parent)))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(
            out.content.contains("the agent that created you"),
            "{}",
            out.content
        );
    }

    #[tokio::test]
    async fn stopping_myself_is_refused() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let out = StopAgentTool
            .invoke(json!({"agent": "me"}), &ctx(dir, me, &[]))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("this agent itself"), "{}", out.content);
    }

    #[tokio::test]
    async fn an_unknown_agent_is_refused_and_says_so() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let out = StopAgentTool
            .invoke(json!({"agent": "ghost"}), &ctx(dir, me, &[]))
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("no running agent matches"), "{}", out.content);
    }

    #[test]
    fn the_permission_rule_admits_a_descendant_and_nothing_else() {
        let me = AgentId::new();
        let child = AgentId::new();
        let outsider = AgentId::new();
        let parent = AgentId::new();

        let entry = |id: &AgentId, ancestors: &[AgentId]| RegistryEntry {
            id: id.clone(),
            name: None,
            group: None,
            pid: 1,
            started_at: Utc::now(),
            provider: "ollama".into(),
            model: "m".into(),
            socket: PathBuf::from("/tmp/x.sock"),
            persona: None,
            ancestors: ancestors.to_vec(),
        };

        // A direct child and a grandchild are ours, whoever is in between.
        assert!(stop_permission(&entry(&child, std::slice::from_ref(&me)), &me, &[], "child").is_ok());
        let grandchild = AgentId::new();
        assert!(stop_permission(
            &entry(&grandchild, &[me.clone(), child.clone()]),
            &me,
            &[],
            "grandchild"
        )
        .is_ok());

        // Someone else's agent, our own parent and ourselves are not.
        let err = stop_permission(&entry(&outsider, &[]), &me, &[], "outsider").unwrap_err();
        assert!(err.contains("you did not create it"), "{err}");
        let err = stop_permission(
            &entry(&parent, &[]),
            &me,
            std::slice::from_ref(&parent),
            "parent",
        )
        .unwrap_err();
        assert!(err.contains("the agent that created you"), "{err}");
        let err = stop_permission(&entry(&me, &[]), &me, &[], "me").unwrap_err();
        assert!(err.contains("this agent itself"), "{err}");
    }
}
