//! stdio MCP transport: spawns a server subprocess and speaks newline-delimited
//! JSON-RPC 2.0 over its stdin/stdout. A background task reads replies and routes
//! them to the awaiting caller by request `id`. Dropping the client kills the
//! child and stops the reader.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;

use crate::config::McpServerConfig;
use crate::error::{AppError, Result};
use crate::mcp::proto::{self, McpToolDef};

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<std::result::Result<Value, String>>>>>;

/// A connected MCP server. Holds the child, the piped stdin, and a background
/// reader task; correlates JSON-RPC replies by `id`.
pub struct McpClient {
    stdin: tokio::sync::Mutex<ChildStdin>,
    pending: Pending,
    next_id: AtomicI64,
    child: Mutex<Child>,
    reader: tokio::task::JoinHandle<()>,
    timeout: Duration,
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.reader.abort();
        if let Ok(mut child) = self.child.lock() {
            let _ = child.start_kill();
        }
    }
}

impl McpClient {
    /// Launch `server` over stdio, perform the MCP handshake (`initialize` →
    /// `notifications/initialized` → `tools/list`), and return the client plus
    /// its advertised tools. The whole handshake is bounded by `timeout_ms`.
    pub async fn connect(
        server: &McpServerConfig,
        timeout_ms: u64,
    ) -> Result<(Arc<McpClient>, Vec<McpToolDef>)> {
        if let Some(t) = server.transport.as_deref() {
            if !t.eq_ignore_ascii_case("stdio") {
                return Err(AppError::mcp(format!(
                    "server '{}': unsupported transport '{t}' (only stdio)",
                    server.name
                )));
            }
        }
        if server.command.trim().is_empty() {
            return Err(AppError::mcp(format!(
                "server '{}': empty command",
                server.name
            )));
        }

        let mut cmd = Command::new(&server.command);
        cmd.args(&server.args);
        for (k, v) in &server.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &server.cwd {
            match shellexpand::full(cwd) {
                Ok(p) => {
                    cmd.current_dir(p.as_ref());
                }
                Err(e) => return Err(AppError::mcp(format!("bad cwd '{cwd}': {e}"))),
            }
        }
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            AppError::mcp(format!("failed to launch '{}': {e}", server.command))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::mcp("child stdin unavailable"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::mcp("child stdout unavailable"))?;

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = pending.clone();
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                if let Ok((Some(id), payload)) = proto::parse_jsonrpc_reply(&line) {
                    let sender = reader_pending
                        .lock()
                        .ok()
                        .and_then(|mut m| m.remove(&id));
                    if let Some(tx) = sender {
                        let _ = tx.send(payload);
                    }
                }
                // Non-reply lines (notifications, requests, parse errors) are ignored.
            }
        });

        let client = Arc::new(McpClient {
            stdin: tokio::sync::Mutex::new(stdin),
            pending,
            next_id: AtomicI64::new(1),
            child: Mutex::new(child),
            reader,
            timeout: Duration::from_millis(timeout_ms),
        });

        let defs = tokio::time::timeout(client.timeout, client.handshake())
            .await
            .map_err(|_| {
                AppError::mcp(format!("server '{}': handshake timed out", server.name))
            })??;
        Ok((client, defs))
    }

    async fn handshake(&self) -> Result<Vec<McpToolDef>> {
        let init_params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "agent-cli",
                "version": env!("CARGO_PKG_VERSION"),
            },
        });
        let _ = self.call("initialize", init_params).await?;
        self.notify("notifications/initialized", json!({})).await?;
        let listed = self.call("tools/list", json!({})).await?;
        Ok(proto::parse_tools_list(&listed))
    }

    /// Send a request and await its reply, correlated by `id` and bounded by the
    /// client timeout. A server-side JSON-RPC error becomes an `AppError::Mcp`.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self
                .pending
                .lock()
                .map_err(|_| AppError::mcp("pending map poisoned"))?;
            map.insert(id, tx);
        }
        let req = proto::jsonrpc_request(id, method, &params);
        self.write_line(&req).await?;

        match tokio::time::timeout(self.timeout, rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(msg))) => Err(AppError::mcp(format!("{method}: {msg}"))),
            Ok(Err(_canceled)) => {
                Err(AppError::mcp(format!("{method}: server closed the connection")))
            }
            Err(_elapsed) => {
                if let Ok(mut map) = self.pending.lock() {
                    map.remove(&id);
                }
                Err(AppError::mcp(format!(
                    "{method}: timed out after {}ms",
                    self.timeout.as_millis()
                )))
            }
        }
    }

    /// Fire-and-forget notification (no reply expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let note = proto::jsonrpc_notification(method, &params);
        self.write_line(&note).await
    }

    async fn write_line(&self, msg: &Value) -> Result<()> {
        let line = format!("{}\n", serde_json::to_string(msg)?);
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| AppError::mcp(format!("write failed: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| AppError::mcp(format!("flush failed: {e}")))?;
        Ok(())
    }
}
