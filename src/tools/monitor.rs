use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::agent::AgentEvent;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct MonitorTool;

#[derive(Debug, Deserialize)]
struct MonitorArgs {
    command: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    run_in_background: bool,
}

#[async_trait]
impl Tool for MonitorTool {
    fn name(&self) -> &'static str {
        "monitor"
    }

    fn description(&self) -> &'static str {
        "Run a long-running shell command and collect its stdout lines as events, returning them when the command exits or the timeout elapses."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command whose stdout lines are streamed as events"},
                "description": {"type": "string", "description": "What is being monitored"},
                "timeout_ms": {"type": "integer", "description": "Kill the command after this many milliseconds"},
                "run_in_background": {"type": "boolean", "description": "Run detached; return immediately (best-effort)"}
            },
            "required": ["command"]
        })
    }

    async fn invoke(&self, args: Value, ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: MonitorArgs = serde_json::from_value(args)?;
        let _ = parsed.description;

        if parsed.run_in_background {
            // Best-effort detached spawn: fire and forget. We cannot return a
            // durable handle in the single-output tool model, so acknowledge.
            let mut cmd = Command::new("bash");
            cmd.arg("-lc").arg(&parsed.command);
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cmd.spawn() {
                Ok(_) => return Ok(ToolOutput::ok("spawned in background".to_string())),
                Err(e) => return Ok(ToolOutput::err(format!("spawn error: {e}"))),
            }
        }

        let to_ms = parsed.timeout_ms.unwrap_or(60_000);
        let mut cmd = Command::new("bash");
        cmd.arg("-lc").arg(&parsed.command);
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::err(format!("spawn error: {e}"))),
        };
        let stdout = child.stdout.take().expect("piped stdout");
        let mut reader = BufReader::new(stdout).lines();

        let mut collected: Vec<String> = Vec::new();
        let timed_out = timeout(Duration::from_millis(to_ms), async {
            while let Ok(Some(line)) = reader.next_line().await {
                collected.push(line.clone());
                if let Some(tx) = &ctx.event_tx {
                    let _ = tx
                        .send(AgentEvent::ToolResult {
                            name: "monitor".to_string(),
                            ok: true,
                            output: line,
                        })
                        .await;
                }
            }
        })
        .await
        .is_err();
        // Best-effort kill on timeout.
        let _ = child.kill().await;
        let _ = child.wait().await;

        let mut body = collected.join("\n");
        if timed_out {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(&format!("...[timed out after {to_ms} ms]"));
            Ok(ToolOutput::err(body))
        } else {
            Ok(ToolOutput::ok(body))
        }
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
            config_source: Default::default(),
            event_tx: None,
        }
    }

    #[tokio::test]
    async fn monitor_collects_output_lines() {
        let out = MonitorTool
            .invoke(
                json!({"command": "printf 'line1\\nline2\\n'", "timeout_ms": 5000}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok, "{}", out.content);
        assert!(out.content.contains("line1"));
        assert!(out.content.contains("line2"));
    }

    #[tokio::test]
    async fn monitor_timeout_returns_partial() {
        let out = MonitorTool
            .invoke(
                json!({"command": "echo before; sleep 5; echo ZZZ_SHOULD_NOT_APPEAR", "timeout_ms": 500}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("before"), "content: {}", out.content);
        assert!(out.content.contains("timed out"), "content: {}", out.content);
        assert!(
            !out.content.contains("ZZZ_SHOULD_NOT_APPEAR"),
            "content: {}",
            out.content
        );
    }
}