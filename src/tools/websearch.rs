use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::Config;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

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
    fn name(&self) -> &'static str {
        "websearch"
    }

    fn description(&self) -> &'static str {
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
        let client = reqwest::Client::new();
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

        let resp = match client.post(&endpoint).json(&req_body).send().await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::err(format!("websearch request failed: {e}"))),
        };
        let status = resp.status();
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => return Ok(ToolOutput::err(format!("websearch read failed: {e}"))),
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
            registry_dir: PathBuf::from("/tmp"),
            config_source: Default::default(),
            event_tx: None,
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
}