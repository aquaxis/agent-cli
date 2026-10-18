use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

use crate::config::Config;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

/// Bound for one websearch exchange (connect, response and body read) — the
/// same total-duration shape `webfetch` uses. The turn model is strictly
/// serial, so an unbounded HTTP call here parks the agent's whole queue: a
/// search endpoint that accepts and then stalls forever would surface as the
/// "peer prompt never starts processing" field shape instead of a timed-out
/// tool result.
const SEARCH_TIMEOUT: Duration = Duration::from_secs(60);

/// One POST to the search endpoint and the response read, bounded by
/// `timeout` across all of it. Error texts are the tool's caller-facing ones
/// ("websearch request failed", "websearch read failed").
async fn search_exchange(
    endpoint: &str,
    req_body: Value,
    timeout: Duration,
) -> std::result::Result<(reqwest::StatusCode, String), String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(endpoint)
        .json(&req_body)
        .send()
        .await
        .map_err(|e| format!("websearch request failed: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("websearch read failed: {e}"))?;
    Ok((status, text))
}

pub struct WebSearchTool {
    cfg: Config,
}

impl WebSearchTool {
    pub fn new(cfg: Config) -> Self {
        Self { cfg }
    }
}

#[derive(Debug, Deserialize)]
struct WebSearchArgs {
    query: String,
    #[serde(default)]
    allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    blocked_domains: Option<Vec<String>>,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "Run a web search and return result entries (title, url, snippet). Requires [tools.websearch] configuration (api key + endpoint)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search query"},
                "allowed_domains": {"type": "array", "items": {"type": "string"}, "description": "Only include results from these domains"},
                "blocked_domains": {"type": "array", "items": {"type": "string"}, "description": "Exclude results from these domains"}
            },
            "required": ["query"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: WebSearchArgs = serde_json::from_value(args)?;
        let ws = &self.cfg.tools.websearch;
        let endpoint = match &ws.endpoint {
            Some(e) => e.clone(),
            None => {
                return Ok(ToolOutput::err(
                    "websearch not configured: set [tools.websearch] endpoint and api_key_env in the config",
                ))
            }
        };
        let key_env = match &ws.api_key_env {
            Some(e) => e.clone(),
            None => {
                return Ok(ToolOutput::err(
                    "websearch not configured: set [tools.websearch] api_key_env in the config",
                ))
            }
        };
        let api_key = match std::env::var(&key_env) {
            Ok(v) if !v.is_empty() => v,
            _ => {
                return Ok(ToolOutput::err(format!(
                    "websearch api key not set in env var {key_env}"
                )))
            }
        };

        let provider = ws.provider.clone().unwrap_or_else(|| "tavily".to_string());
        let req_body = match provider.as_str() {
            "tavily" => json!({
                "api_key": api_key,
                "query": parsed.query,
                "include_domains": parsed.allowed_domains.unwrap_or_default(),
                "exclude_domains": parsed.blocked_domains.unwrap_or_default(),
            }),
            other => {
                return Ok(ToolOutput::err(format!(
                    "unsupported websearch provider: {other} (supported: tavily)"
                )))
            }
        };

        let (status, text) = match search_exchange(&endpoint, req_body, SEARCH_TIMEOUT).await {
            Ok(ok) => ok,
            Err(message) => return Ok(ToolOutput::err(message)),
        };
        if !status.is_success() {
            return Ok(ToolOutput::err(format!(
                "websearch HTTP {status}: {}",
                text.chars().take(500).collect::<String>()
            )));
        }
        let v: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => return Ok(ToolOutput::err(format!("websearch bad response: {e}"))),
        };
        // Tavily returns { results: [ { title, url, content } ] }
        let results = v
            .get("results")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = String::new();
        for r in results {
            let title = r.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let url = r.get("url").and_then(|u| u.as_str()).unwrap_or("");
            let snippet = r
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .replace('\n', " ");
            out.push_str(&format!("- {}: {}\n  {}\n", title, url, snippet));
        }
        if out.is_empty() {
            out = "no results".to_string();
        }
        Ok(ToolOutput::ok(out))
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
            ancestors: Vec::new(),
            spawn_limits: crate::swarm::SpawnLimits::default(),
        }
    }

    fn cfg() -> Config {
        toml::from_str(
            r#"
[provider]
kind = "claude"
[tools]
enabled = ["websearch"]
"#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn websearch_unconfigured_returns_err() {
        let tool = WebSearchTool::new(cfg());
        let out = tool
            .invoke(json!({"query": "rust"}), &ctx())
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("not configured"));
    }

    /// The pin for the search bound's reason to exist: the turn model is
    /// strictly serial, so a search endpoint that accepts and then stalls
    /// forever must end the exchange with the bound firing instead of holding
    /// the head of the queue immortally — the shape webfetch's bound closed
    /// in v0.27.0; websearch was the one tool wait left unbounded.
    ///
    /// The stub first READS a request byte (an immediate write would race
    /// hyper's still-in-flight request and can cancel the connection before
    /// the bound can matter), then declares a body length it never finishes
    /// writing: the body read — the thing the bound exists for — is the only
    /// way out. Two assertions make the test impossible to satisfy by luck:
    /// the exchange must last at least ~the bound window (an instant error
    /// means the bound never mattered) and must end inside the wall-clock
    /// fence (the pre-bound behavior was an immortal call), plus the
    /// caller-facing error text must survive.
    #[tokio::test]
    async fn search_exchange_times_out_on_a_stalled_endpoint() {
        use std::io::{Read, Write};
        use std::time::Instant;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            // Wait for the request before answering (no early-response race).
            let mut first = [0u8; 1];
            let _ = sock.read(&mut first);
            sock.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n{\"res",
            )
            .unwrap();
            sock.flush().unwrap();
            // Hold the rest of the declared body forever: the thread parks
            // rather than shuts down, so the client's bound — not this
            // thread's death — is what ends the exchange. Never joined; one
            // parked thread for the test binary's lifetime.
            loop {
                std::thread::park();
            }
        });
        let _ = server;
        let started = Instant::now();
        // The 10 s fence turns the pre-bound shape (an immortal call) into a
        // test failure instead of a hung run.
        let out = tokio::time::timeout(
            Duration::from_secs(10),
            search_exchange(
                &format!("http://127.0.0.1:{port}/"),
                json!({"api_key": "k", "query": "q"}),
                Duration::from_secs(1),
            ),
        )
        .await;
        let elapsed = started.elapsed();
        let out = out.expect("the exchange must end inside the wall-clock fence");
        // The 1 s window here is 60x smaller than the production constant;
        // the floor excludes instant errors (the bound never fired), the
        // generous ceiling excludes scheduling jitter, not a hang.
        assert!(
            elapsed >= Duration::from_millis(900),
            "expected the bound to fire, not an instant error; took {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(30),
            "expected the bounded timeout, took {elapsed:?}"
        );
        let message = if let Err(e) = out {
            e
        } else {
            panic!("expected the stalled endpoint to fail, not succeed")
        };
        assert!(
            message.contains("websearch request failed")
                || message.contains("websearch read failed"),
            "the caller-facing text must survive: {message}"
        );
    }
}
