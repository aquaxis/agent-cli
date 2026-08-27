use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::agent::AgentEvent;
use crate::ai::ToolSpec;
use crate::config::{Config, ConfigSource};
use crate::error::Result;
use crate::id::AgentId;

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod monitor;
pub mod read;
pub mod send_to;
pub mod spawn;
pub mod webfetch;
pub mod websearch;
pub mod write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub ok: bool,
    pub content: String,
}

impl ToolOutput {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            ok: true,
            content: text.into(),
        }
    }

    pub fn err(text: impl Into<String>) -> Self {
        Self {
            ok: false,
            content: text.into(),
        }
    }
}

pub struct ToolCtx {
    pub self_id: AgentId,
    /// This agent's group, forwarded to the `spawn` tool so a spawned peer
    /// inherits the same group when the tool call does not override it.
    pub group: Option<crate::id::GroupId>,
    pub registry_dir: PathBuf,
    /// This agent's config source (path), forwarded to the `spawn` tool so a
    /// spawned peer inherits the same config file (hence the same registry_dir).
    pub config_source: ConfigSource,
    /// Optional event channel for tools that stream progress (e.g. `monitor`).
    /// Most tools ignore this field.
    pub event_tx: Option<mpsc::Sender<AgentEvent>>,
}

/// Abstract tool callable by the AI.
///
/// `name` is the identifier presented to the LLM, `schema` is the JSON Schema for arguments.
/// `invoke` returns a `ToolOutput` from the given arguments and context.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool identifier (snake_case recommended).
    fn name(&self) -> &'static str;
    /// Brief description presented to the LLM.
    fn description(&self) -> &'static str;
    /// JSON Schema for arguments.
    fn schema(&self) -> Value;
    /// Execute the tool. On failure, return `ToolOutput::err` to the AI.
    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput>;
}

pub struct ToolRegistry {
    pub tools: HashMap<String, Arc<dyn Tool>>,
}

/// Map a tool name from config (possibly a pre-rename legacy name) to its
/// current canonical name. `shell` -> `bash`, `fs_read` -> `read`,
/// `fs_write` -> `write`; everything else passes through unchanged.
fn canonical_tool_name(s: &str) -> &str {
    match s {
        "shell" => "bash",
        "fs_read" => "read",
        "fs_write" => "write",
        other => other,
    }
}

impl ToolRegistry {
    pub fn build(cfg: &Config, allowed: Option<&[String]>, denied: Option<&[String]>) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let timeout_ms = cfg.tools.bash.timeout_ms;
        let max_kb = cfg.tools.bash.max_output_kb;
        let candidates: Vec<(&str, Arc<dyn Tool>)> = vec![
            ("bash", Arc::new(bash::BashTool::new(timeout_ms, max_kb))),
            ("read", Arc::new(read::ReadTool)),
            ("write", Arc::new(write::WriteTool)),
            ("send_to", Arc::new(send_to::SendToTool)),
            ("spawn", Arc::new(spawn::SpawnTool)),
            ("monitor", Arc::new(monitor::MonitorTool)),
            ("edit", Arc::new(edit::EditTool)),
            ("glob", Arc::new(glob::GlobTool)),
            ("grep", Arc::new(grep::GrepTool)),
            ("websearch", Arc::new(websearch::WebSearchTool::new(cfg.clone()))),
            ("webfetch", Arc::new(webfetch::WebFetchTool)),
        ];
        for (name, tool) in candidates {
            // Backward compatibility: accept the pre-rename tool names in the
            // `enabled` list (shell -> bash, fs_read -> read, fs_write -> write)
            // so existing config files keep working after the Claude-Code-aligned
            // rename. Without this, old configs would silently lose those tools.
            if !cfg
                .tools
                .enabled
                .iter()
                .any(|t| canonical_tool_name(t) == name)
            {
                continue;
            }
            if let Some(allow) = allowed {
                if !allow.iter().any(|t| canonical_tool_name(t) == name) {
                    continue;
                }
            }
            if let Some(deny) = denied {
                if deny.iter().any(|t| canonical_tool_name(t) == name) {
                    continue;
                }
            }
            tools.insert(name.to_string(), tool);
        }
        ToolRegistry { tools }
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools
            .values()
            .map(|t| ToolSpec {
                name: t.name().to_string(),
                description: t.description().to_string(),
                schema: t.schema(),
            })
            .collect()
    }

    #[allow(dead_code)]
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tools.keys().cloned().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_cfg() -> Config {
        toml::from_str(crate::config::tests_default_config()).unwrap()
    }

    #[test]
    fn default_registry_registers_renamed_and_new_tools() {
        let cfg = default_cfg();
        let reg = ToolRegistry::build(&cfg, None, None);
        // All implemented candidate tools are registered regardless of enabled
        // list (enabled filtering happens in build()).
        for name in [
            "bash", "read", "write", "send_to", "monitor", "edit", "glob", "grep",
            "websearch", "webfetch",
        ] {
            assert!(reg.get(name).is_some(), "tool not registered: {name}");
        }
    }

    #[test]
    fn default_registry_omits_old_tool_names() {
        let cfg = default_cfg();
        let reg = ToolRegistry::build(&cfg, None, None);
        for old in ["shell", "fs_read", "fs_write"] {
            assert!(reg.get(old).is_none(), "old tool name still registered: {old}");
        }
    }

    #[test]
    fn legacy_enabled_names_activate_renamed_tools() {
        // An existing config file that still lists the pre-rename tool names
        // must keep the renamed tools available (backward compatibility).
        let toml_src = r#"
[provider]
kind = "claude"
[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        let reg = ToolRegistry::build(&cfg, None, None);
        assert!(reg.get("bash").is_some(), "legacy 'shell' should enable bash");
        assert!(reg.get("read").is_some(), "legacy 'fs_read' should enable read");
        assert!(reg.get("write").is_some(), "legacy 'fs_write' should enable write");
        assert!(reg.get("send_to").is_some());
        // Legacy names themselves are not registered as tool keys.
        assert!(reg.get("shell").is_none());
        assert!(reg.get("fs_read").is_none());
    }

    #[test]
    fn legacy_tools_shell_block_parses_as_bash() {
        // A legacy `[tools.shell]` block must still parse (into the bash config)
        // rather than breaking config loading.
        let toml_src = r#"
[provider]
kind = "claude"
[tools]
enabled = ["bash"]
[tools.shell]
timeout_secs  = 60
max_output_kb = 256
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        // `timeout_secs` has no equivalent and is ignored; `max_output_kb` is
        // honored; `timeout_ms` falls back to its default.
        assert_eq!(cfg.tools.bash.max_output_kb, 256);
        assert_eq!(cfg.tools.bash.timeout_ms, 120_000);
    }

    #[test]
    fn default_registry_respects_enabled_filter() {
        let cfg = default_cfg();
        let reg = ToolRegistry::build(&cfg, None, None);
        // The default enabled list does NOT include the network tools.
        assert!(reg.get("bash").is_some());
        assert!(reg.get("websearch").is_some()); // registered (candidate), enabled-independent
        // With an explicit allowlist, only allowlisted tools remain.
        let allow = vec!["read".to_string(), "edit".to_string()];
        let reg2 = ToolRegistry::build(&cfg, Some(&allow), None);
        assert!(reg2.get("read").is_some());
        assert!(reg2.get("edit").is_some());
        assert!(reg2.get("bash").is_none(), "bash should be filtered out by allowlist");
    }
}
