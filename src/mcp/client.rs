//! MCP client generalised over a transport. The handshake, `tools/list`, and
//! `tools/call` logic is shared; only the wire send/receive differs per
//! transport — [`StdioTransport`] (a launched subprocess, defined here) or
//! [`HttpTransport`](crate::mcp::http::HttpTransport) (Streamable HTTP).

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::oneshot;

use crate::config::McpServerConfig;
use crate::error::{AppError, Result};
use crate::mcp::http::HttpTransport;
use crate::mcp::proto::{self, McpToolDef};

type Pending = Arc<Mutex<HashMap<i64, oneshot::Sender<std::result::Result<Value, String>>>>>;

/// A transport backend: sends a JSON-RPC request correlated to `id` and returns
/// its `result` payload, or sends a fire-and-forget notification.
#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn request(&self, id: i64, msg: &Value, timeout: Duration) -> Result<Value>;
    async fn notify(&self, msg: &Value, timeout: Duration) -> Result<()>;
}

/// A connected MCP server, transport-agnostic.
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    next_id: AtomicI64,
    timeout: Duration,
}

impl McpClient {
    /// Connect `server` over its configured transport (`stdio` default, or
    /// `http`), perform the MCP handshake (`initialize` →
    /// `notifications/initialized` → `tools/list`), and return the client plus
    /// its advertised tools. The whole handshake is bounded by `timeout_ms`.
    pub async fn connect(
        server: &McpServerConfig,
        timeout_ms: u64,
    ) -> Result<(Arc<McpClient>, Vec<McpToolDef>)> {
        let timeout = Duration::from_millis(timeout_ms);
        let kind = server.transport.as_deref().unwrap_or("stdio");
        let transport: Box<dyn McpTransport> = if kind.eq_ignore_ascii_case("stdio") {
            Box::new(StdioTransport::spawn(server)?)
        } else if kind.eq_ignore_ascii_case("http") {
            Box::new(HttpTransport::new(server)?)
        } else {
            return Err(AppError::mcp(format!(
                "server '{}': unsupported transport '{kind}' (use 'stdio' or 'http')",
                server.name
            )));
        };

        let client = Arc::new(McpClient {
            transport,
            next_id: AtomicI64::new(1),
            timeout,
        });

        let defs = tokio::time::timeout(timeout, client.handshake())
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

    /// Send a request and return its `result` payload; a server-side error or a
    /// transport failure becomes an `AppError::Mcp` prefixed with the method.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = proto::jsonrpc_request(id, method, &params);
        self.transport
            .request(id, &req, self.timeout)
            .await
            .map_err(|e| match e {
                AppError::Mcp(m) => AppError::mcp(format!("{method}: {m}")),
                other => other,
            })
    }

    /// Fire-and-forget notification (no reply expected).
    pub async fn notify(&self, method: &str, params: Value) -> Result<()> {
        let note = proto::jsonrpc_notification(method, &params);
        self.transport.notify(&note, self.timeout).await
    }
}

// ---------------------------------------------------------------------------
// stdio transport
// ---------------------------------------------------------------------------

/// stdio transport: a launched subprocess speaking newline-delimited JSON-RPC
/// over its stdin/stdout. A background task reads replies and routes them to the
/// awaiting caller by `id`; dropping it kills the child and stops the reader.
struct StdioTransport {
    stdin: tokio::sync::Mutex<ChildStdin>,
    pending: Pending,
    child: Mutex<Child>,
    reader: tokio::task::JoinHandle<()>,
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        self.reader.abort();
        if let Ok(mut child) = self.child.lock() {
            let _ = child.start_kill();
        }
    }
}

impl StdioTransport {
    fn spawn(server: &McpServerConfig) -> Result<Self> {
        if server.command.trim().is_empty() {
            return Err(AppError::mcp(format!(
                "server '{}': transport=stdio requires a command",
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

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::mcp(format!("failed to launch '{}': {e}", server.command)))?;
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
                    let sender = reader_pending.lock().ok().and_then(|mut m| m.remove(&id));
                    if let Some(tx) = sender {
                        let _ = tx.send(payload);
                    }
                }
                // Non-reply lines (notifications, requests, parse errors) are ignored.
            }
        });

        Ok(StdioTransport {
            stdin: tokio::sync::Mutex::new(stdin),
            pending,
            child: Mutex::new(child),
            reader,
        })
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

#[async_trait]
impl McpTransport for StdioTransport {
    async fn request(&self, id: i64, msg: &Value, timeout: Duration) -> Result<Value> {
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self
                .pending
                .lock()
                .map_err(|_| AppError::mcp("pending map poisoned"))?;
            map.insert(id, tx);
        }
        self.write_line(msg).await?;

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(msg))) => Err(AppError::mcp(msg)),
            Ok(Err(_canceled)) => Err(AppError::mcp("server closed the connection")),
            Err(_elapsed) => {
                if let Ok(mut map) = self.pending.lock() {
                    map.remove(&id);
                }
                Err(AppError::mcp(format!(
                    "timed out after {}ms",
                    timeout.as_millis()
                )))
            }
        }
    }

    async fn notify(&self, msg: &Value, _timeout: Duration) -> Result<()> {
        self.write_line(msg).await
    }
}
