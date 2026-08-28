//! Model Context Protocol (MCP) client. agent-cli connects to the servers
//! declared in `[[mcp.servers]]` at startup (stdio transport), discovers their
//! tools via `tools/list`, and registers each as an ordinary [`Tool`] under the
//! namespaced key `mcp__<server>__<tool>`. Invoking such a tool forwards to the
//! server's `tools/call`. Per-server failures are logged and skipped so a bad
//! server never breaks startup.

mod client;
pub mod proto;

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::{default_mcp_init_timeout_ms, McpConfig};
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

use client::McpClient;
use proto::flatten_tool_result;

/// A tool discovered from an MCP server, callable through the normal agent loop.
pub struct McpTool {
    client: Arc<McpClient>,
    /// Server-side tool name (sent in `tools/call`).
    remote_name: String,
    /// Namespaced registry key / spec name (`mcp__<server>__<tool>`).
    name: String,
    description: String,
    schema: Value,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn schema(&self) -> Value {
        self.schema.clone()
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let params = json!({ "name": self.remote_name, "arguments": args });
        match self.client.call("tools/call", params).await {
            Ok(result) => Ok(flatten_tool_result(&result)),
            // A transport/protocol failure is surfaced to the model as a tool
            // error, not a hard failure of the agent.
            Err(e) => Ok(ToolOutput::err(format!("mcp call failed: {e}"))),
        }
    }
}

fn mcp_description(server: &str, desc: &str) -> String {
    if desc.trim().is_empty() {
        format!("[mcp:{server}]")
    } else {
        format!("[mcp:{server}] {desc}")
    }
}

/// Connect to every enabled server and return the tools to register. Never
/// errors as a whole: a server that fails to spawn, handshake, or list is logged
/// and dropped (the remaining servers and the built-in tools are unaffected).
pub async fn connect_all(cfg: &McpConfig) -> Vec<Arc<dyn Tool>> {
    let timeout_ms = cfg
        .init_timeout_ms
        .unwrap_or_else(default_mcp_init_timeout_ms);
    let mut out: Vec<Arc<dyn Tool>> = Vec::new();
    for server in cfg.servers.iter().filter(|s| s.enabled) {
        match McpClient::connect(server, timeout_ms).await {
            Ok((client, defs)) => {
                let count = defs.len();
                for def in defs {
                    out.push(Arc::new(McpTool {
                        client: client.clone(),
                        name: proto::mcp_tool_name(&server.name, &def.name),
                        remote_name: def.name,
                        description: mcp_description(&server.name, &def.description),
                        schema: def.input_schema,
                    }));
                }
                tracing::info!("mcp server '{}' connected: {count} tool(s)", server.name);
            }
            Err(e) => tracing::warn!("mcp server '{}' skipped: {e}", server.name),
        }
    }
    out
}

/// The outcome of probing one configured server (for `mcp list` / `doctor`).
pub struct ServerProbe {
    pub name: String,
    pub enabled: bool,
    /// Namespaced tool names on success, or an error message.
    pub outcome: std::result::Result<Vec<String>, String>,
}

/// Connect to each configured server (including disabled ones, reported as
/// such) and report its reachability and discovered tools. Used by the `mcp
/// list` subcommand and the `doctor` check.
pub async fn probe_all(cfg: &McpConfig) -> Vec<ServerProbe> {
    let timeout_ms = cfg
        .init_timeout_ms
        .unwrap_or_else(default_mcp_init_timeout_ms);
    let mut out = Vec::new();
    for server in &cfg.servers {
        if !server.enabled {
            out.push(ServerProbe {
                name: server.name.clone(),
                enabled: false,
                outcome: Ok(Vec::new()),
            });
            continue;
        }
        let outcome = match McpClient::connect(server, timeout_ms).await {
            Ok((_client, defs)) => Ok(defs
                .iter()
                .map(|d| proto::mcp_tool_name(&server.name, &d.name))
                .collect()),
            Err(e) => Err(e.to_string()),
        };
        out.push(ServerProbe {
            name: server.name.clone(),
            enabled: true,
            outcome,
        });
    }
    out
}
