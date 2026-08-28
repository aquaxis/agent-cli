use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct BashTool {
    pub default_timeout_ms: u64,
    pub max_output_kb: u64,
}

impl BashTool {
    pub fn new(default_timeout_ms: u64, max_output_kb: u64) -> Self {
        Self {
            default_timeout_ms,
            max_output_kb,
        }
    }
}

#[derive(Debug, Deserialize)]
struct BashArgs {
    command: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    run_in_background: bool,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command. Returns stdout, stderr and exit_code as JSON."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "The bash command to execute"},
                "description": {"type": "string", "description": "Clear, concise description of what the command does"},
                "timeout_ms": {"type": "integer", "description": "Max execution time in milliseconds"},
                "run_in_background": {"type": "boolean", "description": "Run detached in the background (returns immediately)"}
            },
            "required": ["command"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: BashArgs = serde_json::from_value(args)?;
        if parsed.run_in_background {
            return Ok(ToolOutput::err(
                "run_in_background is not supported by the bash tool; use the monitor tool",
            ));
        }
        let _ = parsed.description; // informational only
        let to = parsed.timeout_ms.unwrap_or(self.default_timeout_ms);
        let mut cmd = Command::new("bash");
        cmd.arg("-lc").arg(&parsed.command);
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let fut = cmd.output();
        let output = match timeout(Duration::from_millis(to), fut).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                return Ok(ToolOutput::err(format!("spawn error: {e}")));
            }
            Err(_) => {
                return Ok(ToolOutput::err(format!(
                    "timed out after {to} ms: {}",
                    parsed.command
                )));
            }
        };

        let max_bytes = (self.max_output_kb * 1024) as usize;
        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stdout.len() > max_bytes {
            stdout.truncate(max_bytes);
            stdout.push_str("\n...[truncated]");
        }
        if stderr.len() > max_bytes {
            stderr.truncate(max_bytes);
            stderr.push_str("\n...[truncated]");
        }
        let exit_code = output.status.code().unwrap_or(-1);
        let body = serde_json::to_string(&json!({
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
        }))?;
        Ok(ToolOutput {
            ok: output.status.success(),
            content: body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::AgentId;
    use std::path::PathBuf;

    fn ctx() -> ToolCtx {
        ToolCtx {
            self_id: AgentId::new(),
            group: None,
            registry_dir: PathBuf::from("/tmp"),
            config_source: Default::default(),
            event_tx: None,
        }
    }

    #[tokio::test]
    async fn echo_works() {
        let tool = BashTool::new(5_000, 64);
        let out = tool
            .invoke(json!({"command": "echo hello"}), &ctx())
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("hello"));
    }

    #[tokio::test]
    async fn timeout_triggers() {
        let tool = BashTool::new(1_000, 64);
        let out = tool
            .invoke(json!({"command": "sleep 5", "timeout_ms": 1000}), &ctx())
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("timed out"));
    }

    #[tokio::test]
    async fn run_in_background_errors() {
        let tool = BashTool::new(5_000, 64);
        let out = tool
            .invoke(
                json!({"command": "echo hi", "run_in_background": true}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("monitor"));
    }
}