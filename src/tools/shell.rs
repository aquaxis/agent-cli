use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct ShellTool {
    pub default_timeout_secs: u64,
    pub max_output_kb: u64,
}

impl ShellTool {
    pub fn new(default_timeout_secs: u64, max_output_kb: u64) -> Self {
        Self {
            default_timeout_secs,
            max_output_kb,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ShellArgs {
    cmd: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command using `bash -lc <cmd>`. Returns stdout, stderr and exit_code."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "cmd": {"type": "string", "description": "Command to execute"},
                "cwd": {"type": "string", "description": "Working directory"},
                "timeout_secs": {"type": "integer", "description": "Timeout in seconds"},
            },
            "required": ["cmd"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: ShellArgs = serde_json::from_value(args)?;
        let to = parsed.timeout_secs.unwrap_or(self.default_timeout_secs);
        let mut cmd = Command::new("bash");
        cmd.arg("-lc").arg(&parsed.cmd);
        if let Some(dir) = &parsed.cwd {
            cmd.current_dir(dir);
        }
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let fut = cmd.output();
        let output = match timeout(Duration::from_secs(to), fut).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => {
                return Ok(ToolOutput::err(format!("spawn error: {e}")));
            }
            Err(_) => {
                return Ok(ToolOutput::err(format!(
                    "timed out after {to} seconds: {}",
                    parsed.cmd
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
            registry_dir: PathBuf::from("/tmp"),
        }
    }

    #[tokio::test]
    async fn echo_works() {
        let tool = ShellTool::new(5, 64);
        let out = tool
            .invoke(json!({"cmd": "echo hello"}), &ctx())
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("hello"));
    }

    #[tokio::test]
    async fn timeout_triggers() {
        let tool = ShellTool::new(1, 64);
        let out = tool
            .invoke(json!({"cmd": "sleep 5", "timeout_secs": 1}), &ctx())
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("timed out"));
    }
}
