//! HTTP (Streamable HTTP) MCP transport. Each JSON-RPC request is POSTed to the
//! server URL; the reply arrives as either a single `application/json` body or a
//! `text/event-stream` (SSE) stream of messages, from which the message matching
//! the request `id` is selected. The `Mcp-Session-Id` returned by `initialize`
//! and the negotiated protocol version are echoed on subsequent requests.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;

use crate::ai::stream::SseAccumulator;
use crate::config::McpServerConfig;
use crate::error::{AppError, Result};
use crate::mcp::client::McpTransport;
use crate::mcp::proto;

pub struct HttpTransport {
    client: reqwest::Client,
    url: String,
    /// Static headers from config (`Content-Type`/`Accept` are added per request).
    static_pairs: Vec<(String, String)>,
    /// Resolved once from `api_key_env`; sent as `Authorization: Bearer <value>`.
    bearer: Option<String>,
    session_id: Mutex<Option<String>>,
    protocol_version: Mutex<Option<String>>,
}

impl HttpTransport {
    pub fn new(server: &McpServerConfig) -> Result<Self> {
        let url = server
            .url
            .clone()
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| {
                AppError::mcp(format!(
                    "server '{}': transport=http requires a url",
                    server.name
                ))
            })?;

        let bearer = match &server.api_key_env {
            Some(env) => Some(std::env::var(env).map_err(|_| {
                AppError::mcp(format!(
                    "server '{}': api_key_env '{env}' is not set",
                    server.name
                ))
            })?),
            None => None,
        };

        let static_pairs: Vec<(String, String)> = server
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| AppError::mcp(format!("http client build failed: {e}")))?;

        Ok(HttpTransport {
            client,
            url,
            static_pairs,
            bearer,
            session_id: Mutex::new(None),
            protocol_version: Mutex::new(None),
        })
    }

    fn headers_for_request(&self) -> Vec<(String, String)> {
        let sid = self.session_id.lock().ok().and_then(|g| g.clone());
        let pv = self.protocol_version.lock().ok().and_then(|g| g.clone());
        proto::http_request_headers(
            &self.static_pairs,
            self.bearer.as_deref(),
            sid.as_deref(),
            pv.as_deref(),
        )
    }

    fn apply_headers(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        for (k, v) in self.headers_for_request() {
            req = req.header(k, v);
        }
        req
    }
}

impl Drop for HttpTransport {
    fn drop(&mut self) {
        // Best-effort session termination; ignore all failures.
        let sid = self.session_id.lock().ok().and_then(|g| g.clone());
        if let Some(sid) = sid {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let client = self.client.clone();
                let url = self.url.clone();
                handle.spawn(async move {
                    let _ = client.delete(&url).header("Mcp-Session-Id", sid).send().await;
                });
            }
        }
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn request(&self, id: i64, msg: &Value, timeout: Duration) -> Result<Value> {
        let body = serde_json::to_string(msg)?;
        let fut = async {
            let req = self.apply_headers(self.client.post(&self.url)).body(body);
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::mcp(format!("http post: {e}")))?;

            // Capture the session id from any response that carries one.
            if let Some(sid) = resp
                .headers()
                .get("mcp-session-id")
                .and_then(|v| v.to_str().ok())
            {
                if let Ok(mut g) = self.session_id.lock() {
                    *g = Some(sid.to_string());
                }
            }

            let status = resp.status();
            let ctype = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            if !status.is_success() {
                let snippet = resp.text().await.unwrap_or_default();
                return Err(AppError::mcp(format!(
                    "HTTP {status}: {}",
                    snippet.chars().take(300).collect::<String>()
                )));
            }

            let reply = if ctype.contains("text/event-stream") {
                self.read_sse_reply(resp, id).await?
            } else {
                let text = resp
                    .text()
                    .await
                    .map_err(|e| AppError::mcp(format!("read body: {e}")))?;
                let v: Value = serde_json::from_str(&text)
                    .map_err(|e| AppError::mcp(format!("invalid JSON reply: {e}")))?;
                proto::select_reply_by_id(&[v], id)
            };

            match reply {
                Some(Ok(result)) => {
                    // Capture the negotiated protocol version (present on the
                    // initialize result) for subsequent requests.
                    if let Some(pv) = result.get("protocolVersion").and_then(|v| v.as_str()) {
                        if let Ok(mut g) = self.protocol_version.lock() {
                            *g = Some(pv.to_string());
                        }
                    }
                    Ok(result)
                }
                Some(Err(m)) => Err(AppError::mcp(m)),
                None => Err(AppError::mcp("no reply matching request id")),
            }
        };

        tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| AppError::mcp(format!("timed out after {}ms", timeout.as_millis())))?
    }

    async fn notify(&self, msg: &Value, timeout: Duration) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let fut = async {
            let req = self.apply_headers(self.client.post(&self.url)).body(body);
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::mcp(format!("http post: {e}")))?;
            if !resp.status().is_success() {
                return Err(AppError::mcp(format!("HTTP {}", resp.status())));
            }
            Ok(())
        };
        tokio::time::timeout(timeout, fut)
            .await
            .map_err(|_| AppError::mcp("notify timed out"))?
    }
}

impl HttpTransport {
    /// Read an SSE response stream, returning the reply matching `id` as soon as
    /// it appears (interim notifications are ignored).
    async fn read_sse_reply(
        &self,
        resp: reqwest::Response,
        id: i64,
    ) -> Result<Option<std::result::Result<Value, String>>> {
        let mut acc = SseAccumulator::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| AppError::mcp(format!("stream error: {e}")))?;
            match std::str::from_utf8(&bytes) {
                Ok(s) => acc.push(s),
                Err(e) => return Err(AppError::mcp(format!("invalid utf-8 in stream: {e}"))),
            }
            let frames = acc.drain_frames();
            if frames.is_empty() {
                continue;
            }
            let msgs = proto::sse_frames_to_messages(&frames);
            if let Some(reply) = proto::select_reply_by_id(&msgs, id) {
                return Ok(Some(reply));
            }
        }
        Ok(None)
    }
}
