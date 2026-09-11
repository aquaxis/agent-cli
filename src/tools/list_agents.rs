use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::Result;
use crate::ipc::registry;
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// LLM-callable tool that lists the agents this agent created. Read-only: it is
/// one registry scan, and dead peers are already gone because the registry
/// prunes them as it reads. It shows only this agent's own tree — peers created
/// by someone else are not this agent's to manage.
pub struct ListAgentsTool;

#[derive(Debug, Deserialize)]
struct ListArgs {
    /// `"children"` for direct children only, `"subtree"` (the default) for
    /// everything below this agent.
    #[serde(default)]
    scope: Option<String>,
}

#[async_trait]
impl Tool for ListAgentsTool {
    fn name(&self) -> &str {
        "list_agents"
    }

    fn description(&self) -> &str {
        "List the agents you created and are responsible for: id, name, provider/model, group and depth, and whether each is a direct child or a deeper descendant. Use scope=children for your direct children only. Reach one with send_to, stop one with stop_agent."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["children", "subtree"],
                    "description": "children = your direct children only; subtree (default) = every agent below you."
                }
            }
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: ListArgs = serde_json::from_value(args)?;
        let children_only = matches!(parsed.scope.as_deref(), Some("children"));

        let rows = match registry::descendants_of(&ctx.registry_dir, &ctx.self_id) {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::err(format!("could not read the registry: {e}"))),
        };
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|(_, distance)| !children_only || *distance == 1)
            .collect();

        if rows.is_empty() {
            // Nothing to manage is an answer, not a failure.
            return Ok(ToolOutput::ok(if children_only {
                "no child agents".to_string()
            } else {
                "no agents below this one".to_string()
            }));
        }

        // Names of everything live, so a descendant can be described by the
        // agent that owns it rather than by a bare id.
        let live: Vec<_> = registry::list_entries(&ctx.registry_dir).unwrap_or_default();
        let label_of = |id: &crate::id::AgentId| -> String {
            live.iter()
                .find(|e| e.id.as_str() == id.as_str())
                .and_then(|e| e.name.clone())
                .unwrap_or_else(|| id.to_string())
        };

        let mut out = String::new();
        for (entry, distance) in &rows {
            let relation = if *distance == 1 {
                "child".to_string()
            } else {
                match entry.parent() {
                    // The agent in between may have exited; its child keeps
                    // running and is still this agent's to stop.
                    Some(parent) if !live.iter().any(|e| e.id.as_str() == parent.as_str()) => {
                        format!("descendant (via {}, no longer running)", label_of(parent))
                    }
                    Some(parent) => format!("descendant (via {})", label_of(parent)),
                    None => "descendant".to_string(),
                }
            };
            out.push_str(&format!(
                "{}  name={}  provider={} model={}  group={}  depth={}  {}\n",
                entry.id,
                entry.name.as_deref().unwrap_or("-"),
                entry.provider,
                entry.model,
                entry
                    .group
                    .as_ref()
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| "-".into()),
                entry.depth(),
                relation
            ));
        }
        Ok(ToolOutput::ok(out.trim_end().to_string()))
    }
}

/// Registry fixtures shared by the agent-management tools' tests.
#[cfg(test)]
pub(crate) mod tests_support {
    use crate::id::AgentId;
    use crate::ipc::registry::{RegistryEntry, RegistryHandle};
    use crate::swarm::SpawnLimits;
    use crate::tools::ToolCtx;
    use chrono::Utc;
    use std::path::{Path, PathBuf};

    /// Register an agent whose pid is alive (this process') and whose socket
    /// file exists, so `list_entries` keeps it. `ancestors` is the chain, root
    /// first.
    pub(crate) async fn register(dir: &Path, name: &str, ancestors: &[AgentId]) -> AgentId {
        let id = AgentId::new();
        let socket = dir.join(format!("{}.sock", id.as_str()));
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(&socket, b"").unwrap();
        let entry = RegistryEntry {
            id: id.clone(),
            name: Some(name.to_string()),
            group: None,
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "ollama".into(),
            model: "test-model".into(),
            socket,
            persona: None,
            ancestors: ancestors.to_vec(),
        };
        let handle = RegistryHandle::register(dir, &entry).await.unwrap();
        // Keep the files after the handle drops: this is a fixture, not a run.
        std::mem::forget(handle);
        id
    }

    pub(crate) fn ctx(dir: &Path, me: AgentId, ancestors: &[AgentId]) -> ToolCtx {
        ToolCtx {
            self_id: me,
            group: None,
            registry_dir: PathBuf::from(dir),
            config_source: Default::default(),
            event_tx: None,
            ancestors: ancestors.to_vec(),
            spawn_limits: SpawnLimits {
                max_children: 4,
                max_depth: 2,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::{ctx, register};
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn lists_only_this_agent_s_own_tree() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let child = register(dir, "child", std::slice::from_ref(&me)).await;
        let _grandchild = register(dir, "grandchild", &[me.clone(), child.clone()]).await;
        // A peer someone else created, and the agent that created it.
        let other = register(dir, "other", &[]).await;
        let _others_child = register(dir, "others-child", std::slice::from_ref(&other)).await;

        let out = ListAgentsTool
            .invoke(json!({}), &ctx(dir, me.clone(), &[]))
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("name=child"), "{}", out.content);
        assert!(out.content.contains("name=grandchild"), "{}", out.content);
        assert!(
            !out.content.contains("name=other"),
            "another agent's tree must not appear: {}",
            out.content
        );
        assert!(out.content.contains("depth=2"), "{}", out.content);
        assert!(out.content.contains("child"), "{}", out.content);
        assert!(
            out.content.contains("descendant (via child)"),
            "a grandchild is described through the child that owns it: {}",
            out.content
        );
    }

    #[tokio::test]
    async fn scope_children_stops_at_the_first_level() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let child = register(dir, "child", std::slice::from_ref(&me)).await;
        let _grandchild = register(dir, "grandchild", &[me.clone(), child.clone()]).await;

        let out = ListAgentsTool
            .invoke(json!({"scope": "children"}), &ctx(dir, me, &[]))
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("name=child"));
        assert!(!out.content.contains("name=grandchild"), "{}", out.content);
    }

    #[tokio::test]
    async fn an_empty_tree_is_an_answer_not_an_error() {
        let tmp = TempDir::new().unwrap();
        let me = register(tmp.path(), "me", &[]).await;
        let out = ListAgentsTool
            .invoke(json!({}), &ctx(tmp.path(), me, &[]))
            .await
            .unwrap();
        assert!(out.ok, "no children is not a failure");
        assert!(out.content.contains("no agents below this one"));
    }

    #[tokio::test]
    async fn a_descendant_whose_parent_has_gone_is_still_listed() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let me = register(dir, "me", &[]).await;
        let child = register(dir, "child", std::slice::from_ref(&me)).await;
        let _grandchild = register(dir, "grandchild", &[me.clone(), child.clone()]).await;
        // The middle agent exits: its files go, the grandchild keeps running.
        std::fs::remove_file(dir.join(format!("{}.json", child.as_str()))).unwrap();

        let out = ListAgentsTool
            .invoke(json!({}), &ctx(dir, me, &[]))
            .await
            .unwrap();
        assert!(out.ok);
        assert!(
            out.content.contains("name=grandchild"),
            "an orphaned descendant is still ours to manage: {}",
            out.content
        );
        assert!(
            out.content.contains("no longer running"),
            "and it is marked as such: {}",
            out.content
        );
    }
}
