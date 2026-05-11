use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::ToolSpec;
use crate::config::Config;
use crate::error::Result;
use crate::id::AgentId;

pub mod fs_read;
pub mod fs_write;
pub mod send_to;
pub mod shell;

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
    pub registry_dir: PathBuf,
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

impl ToolRegistry {
    pub fn build(cfg: &Config, allowed: Option<&[String]>, denied: Option<&[String]>) -> Self {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let timeout = cfg.tools.shell.timeout_secs;
        let max_kb = cfg.tools.shell.max_output_kb;
        let candidates: Vec<(&str, Arc<dyn Tool>)> = vec![
            ("shell", Arc::new(shell::ShellTool::new(timeout, max_kb))),
            ("fs_read", Arc::new(fs_read::FsReadTool)),
            ("fs_write", Arc::new(fs_write::FsWriteTool)),
            ("send_to", Arc::new(send_to::SendToTool)),
        ];
        for (name, tool) in candidates {
            if !cfg.tools.enabled.iter().any(|t| t == name) {
                continue;
            }
            if let Some(allow) = allowed {
                if !allow.iter().any(|t| t == name) {
                    continue;
                }
            }
            if let Some(deny) = denied {
                if deny.iter().any(|t| t == name) {
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
