use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

pub mod claude;
pub mod codex;
pub mod llamacpp;
pub mod ollama;
pub mod opencode;
pub mod stream;
pub mod tool_bridge;

/// Capabilities provided by the backend.
///
/// REPL and `selftest` use this information for warnings or alternative display of unsupported features.
#[derive(Debug, Clone, Copy, Default)]
pub struct Capabilities {
    /// Whether streaming responses are supported.
    pub streaming: bool,
    /// Whether `tool_use` / function calling can be natively triggered.
    pub tool_use: bool,
    /// Whether thinking blocks (introspection steps) can be received on a separate stream.
    pub thinking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: String,
    },
    /// Tool execution result injected synchronously from outside
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Thinking {
        text: String,
    },
    Text {
        delta: String,
    },
    ToolUse {
        id: String,
        name: String,
        args: serde_json::Value,
    },
    Done,
    Error {
        message: String,
    },
}

pub type EventStream<'a> = Pin<Box<dyn Stream<Item = ProviderEvent> + Send + 'a>>;

/// AI backend abstraction. Each backend returns a `ProviderEvent` stream
/// via `complete_stream`, allowing the upper Agent conversation loop to
/// drive dialog and tool calls without knowing backend-specific representations.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Backend identifier. `"claude"` / `"codex"` / `"ollama"` / `"opencode"` / `"llama.cpp"`.
    fn name(&self) -> &'static str;
    /// Capabilities provided by this backend.
    fn capabilities(&self) -> Capabilities;
    /// Currently used model name.
    fn model(&self) -> &str;
    /// Return a streaming `ProviderEvent` from the given conversation history and tool definitions.
    async fn complete_stream(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<EventStream<'_>>;
}

pub fn build(cfg: &Config, source: &ConfigSource) -> Result<Box<dyn Provider>> {
    let kind = cfg.provider.kind.as_str();
    match kind {
        "claude" => Ok(Box::new(claude::ClaudeProvider::from_config(cfg, source)?)),
        "codex" => Ok(Box::new(codex::CodexProvider::from_config(cfg, source)?)),
        "ollama" => Ok(Box::new(ollama::OllamaProvider::from_config(cfg, source)?)),
        "opencode" => Ok(Box::new(opencode::OpenCodeProvider::from_config(
            cfg, source,
        )?)),
        "llama.cpp" => Ok(Box::new(llamacpp::LlamaCppProvider::from_config(
            cfg, source,
        )?)),
        other => Err(AppError::provider(other, "unknown provider kind")),
    }
}

pub const SUPPORTED: &[&str] = &["claude", "codex", "ollama", "opencode", "llama.cpp"];

/// Diagnostic information for provider HTTP errors (FR-09-3 / design doc 5.1).
///
/// When a 4xx/5xx response is received, this formats all context known to the
/// provider (resolved config file path, `api_key_env` name, masked API key,
/// `request_id`, and pattern-specific hints) for user-friendly display.
#[derive(Debug, Clone)]
pub struct ProviderError {
    pub provider: String,
    pub status: Option<u16>,
    pub status_text: Option<String>,
    pub body: String,
    pub request_id: Option<String>,
    pub config_path: Option<PathBuf>,
    pub api_key_env: Option<String>,
    pub api_key_mask: Option<String>,
    pub hint: Option<String>,
}

impl ProviderError {
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            status: None,
            status_text: None,
            body: String::new(),
            request_id: None,
            config_path: None,
            api_key_env: None,
            api_key_mask: None,
            hint: None,
        }
    }

    pub fn with_http(mut self, status: u16, status_text: impl Into<String>) -> Self {
        self.status = Some(status);
        self.status_text = Some(status_text.into());
        self
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn with_context(mut self, ctx: &ProviderContext) -> Self {
        self.config_path = Some(ctx.config_path.clone());
        self.api_key_env = ctx.api_key_env.clone();
        self.api_key_mask = ctx.api_key_mask.clone();
        self
    }

    /// Analyze the response pattern and embed a hint message.
    pub fn detect_hint(mut self) -> Self {
        self.hint = derive_hint(self.status, &self.body);
        self
    }

    /// Return as a multi-line summary string for use as the `AppError::provider(...)` payload.
    pub fn into_app_error(self) -> AppError {
        let provider = self.provider.clone();
        AppError::provider(provider, self.to_string())
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Line 1: HTTP status summary
        match (self.status, &self.status_text) {
            (Some(code), Some(text)) => writeln!(f, "HTTP {code} {text}")?,
            (Some(code), None) => writeln!(f, "HTTP {code}")?,
            (None, Some(text)) => writeln!(f, "{text}")?,
            (None, None) => {}
        }
        if let Some(rid) = &self.request_id {
            writeln!(f, "  request_id : {rid}")?;
        }
        if let Some(p) = &self.config_path {
            writeln!(f, "  config     : {}", p.display())?;
        }
        match (&self.api_key_env, &self.api_key_mask) {
            (Some(env), Some(mask)) if !mask.is_empty() => {
                writeln!(f, "  api_key_env: {env} ({mask})")?
            }
            (Some(env), _) => writeln!(f, "  api_key_env: {env} (not set)")?,
            _ => {}
        }
        if !self.body.is_empty() {
            // Body can be very long, so collapse to 1 line and truncate at 1KB
            let one_line = self.body.replace('\n', " ");
            let trimmed: String = one_line.chars().take(1024).collect();
            writeln!(f, "  detail     : {trimmed}")?;
        }
        if let Some(hint) = &self.hint {
            writeln!(f, "  hint       : {hint}")?;
        }
        Ok(())
    }
}

/// Per-provider diagnostic context (resolved config file path, `api_key_env`, and key mask).
#[derive(Debug, Clone)]
pub struct ProviderContext {
    pub config_path: PathBuf,
    pub api_key_env: Option<String>,
    pub api_key_mask: Option<String>,
}

impl ProviderContext {
    pub fn new(
        source: &ConfigSource,
        api_key_env: Option<String>,
        api_key_value: Option<&str>,
    ) -> Self {
        let api_key_mask = api_key_value.map(crate::config::mask_api_key);
        Self {
            config_path: source.path.clone(),
            api_key_env,
            api_key_mask,
        }
    }
}

/// Return a "specific pattern -> remediation hint" mapping based on HTTP status and response body.
pub fn derive_hint(status: Option<u16>, body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    if lower.contains("credit balance is too low") {
        return Some(
            "Your Anthropic account credit balance is too low. \
             Check or purchase credits at https://console.anthropic.com/settings/billing, \
             or set a different account's API key in the environment variable pointed to by `api_key_env`."
                .to_string(),
        );
    }
    if status == Some(401)
        || lower.contains("invalid_api_key")
        || lower.contains("authentication_error")
        || lower.contains("invalid x-api-key")
    {
        return Some(
            "The API key is invalid or has been revoked. \
             Verify the value of the environment variable pointed to by `api_key_env`, \
             or reissue the key from the provider's console."
                .to_string(),
        );
    }
    if status == Some(429) || lower.contains("rate_limit") || lower.contains("rate limit") {
        return Some(
            "Rate limit reached. Wait a few minutes before retrying, \
             or switch to lower-frequency calls."
                .to_string(),
        );
    }
    if matches!(status, Some(s) if (500..600).contains(&s)) {
        return Some(
            "A temporary provider-side outage is suspected. \
             Wait a while before retrying."
                .to_string(),
        );
    }
    None
}

/// Extract `request_id` from response headers or body JSON.
///
/// - Headers: `request-id` / `x-request-id` take priority (case-insensitive).
/// - Body JSON: tries top-level `request_id`, then `error.request_id`, then `id` in order.
pub fn extract_request_id(headers: &reqwest::header::HeaderMap, body: &str) -> Option<String> {
    for name in ["request-id", "x-request-id"] {
        if let Some(v) = headers.get(name) {
            if let Ok(s) = v.to_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        for path in [
            &["request_id"][..],
            &["error", "request_id"][..],
            &["id"][..],
        ] {
            let mut cur = &value;
            let mut ok = true;
            for key in path {
                match cur.get(*key) {
                    Some(v) => cur = v,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                if let Some(s) = cur.as_str() {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn hint_for_credit_balance_too_low() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."}}"#;
        let hint = derive_hint(Some(400), body).expect("hint");
        assert!(hint.contains("credit balance"));
        assert!(hint.contains("billing"));
    }

    #[test]
    fn hint_for_authentication_error() {
        let body = r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let hint = derive_hint(Some(401), body).expect("hint");
        assert!(hint.contains("API key"));
    }

    #[test]
    fn hint_for_rate_limit() {
        let hint = derive_hint(Some(429), "rate_limit_exceeded").expect("hint");
        assert!(hint.contains("Rate limit"));
    }

    #[test]
    fn hint_for_server_error() {
        let hint = derive_hint(Some(503), "service unavailable").expect("hint");
        assert!(hint.contains("temporary"));
    }

    #[test]
    fn hint_none_for_unknown_400() {
        // A 400 that matches neither "credit balance too low" nor "authentication error" yields no hint.
        assert!(derive_hint(Some(400), "something else").is_none());
    }

    #[test]
    fn extract_request_id_from_header() {
        let mut h = HeaderMap::new();
        h.insert("request-id", HeaderValue::from_static("req_abc123"));
        let rid = extract_request_id(&h, "{}").expect("rid");
        assert_eq!(rid, "req_abc123");
    }

    #[test]
    fn extract_request_id_from_x_header_when_no_request_id() {
        let mut h = HeaderMap::new();
        h.insert("x-request-id", HeaderValue::from_static("x_xyz_999"));
        let rid = extract_request_id(&h, "{}").expect("rid");
        assert_eq!(rid, "x_xyz_999");
    }

    #[test]
    fn extract_request_id_from_body_when_no_header() {
        let h = HeaderMap::new();
        let body = r#"{"type":"error","error":{},"request_id":"req_011Caej"}"#;
        let rid = extract_request_id(&h, body).expect("rid");
        assert_eq!(rid, "req_011Caej");
    }

    #[test]
    fn extract_request_id_returns_none_when_missing() {
        let h = HeaderMap::new();
        assert!(extract_request_id(&h, "not-json").is_none());
        assert!(extract_request_id(&h, "{}").is_none());
    }

    #[test]
    fn provider_error_display_contains_all_fields() {
        let pe = ProviderError::new("claude")
            .with_http(400, "Bad Request")
            .with_body(r#"{"error":{"message":"Your credit balance is too low to access the Anthropic API."}}"#)
            .with_request_id(Some("req_abc".to_string()))
            .with_context(&ProviderContext {
                config_path: PathBuf::from("/home/u/.config/agent-cli/config.toml"),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_key_mask: Some("sk-a...nQAA".to_string()),
            })
            .detect_hint();
        let s = pe.to_string();
        assert!(s.contains("HTTP 400"));
        assert!(s.contains("req_abc"));
        assert!(s.contains("/home/u/.config/agent-cli/config.toml"));
        assert!(s.contains("ANTHROPIC_API_KEY"));
        assert!(s.contains("sk-a...nQAA"));
        assert!(s.contains("credit balance"));
        // Prevent credential leakage: unmasked key value must not be included
        assert!(!s.contains("sk-ant-fullkey"));
    }

    #[test]
    fn provider_error_display_marks_unset_key() {
        let pe = ProviderError::new("claude")
            .with_http(401, "Unauthorized")
            .with_context(&ProviderContext {
                config_path: PathBuf::from("/c/agent-cli/config.toml"),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_key_mask: None,
            });
        let s = pe.to_string();
        assert!(s.contains("ANTHROPIC_API_KEY (not set)"));
    }
}

#[cfg(test)]
pub mod testing {
    //! Test helper: `MockProvider` that returns scripted `ProviderEvent` sequences.
    use std::sync::Mutex;

    use super::*;

    pub struct MockProvider {
        pub model: String,
        scripts: Mutex<Vec<Vec<ProviderEvent>>>,
    }

    impl MockProvider {
        /// `scripts[i]` is the event sequence emitted on the i-th `complete_stream` call.
        pub fn new(scripts: Vec<Vec<ProviderEvent>>) -> Self {
            Self {
                model: "mock".into(),
                scripts: Mutex::new(scripts),
            }
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                streaming: true,
                tool_use: true,
                thinking: false,
            }
        }

        fn model(&self) -> &str {
            &self.model
        }

        async fn complete_stream(
            &self,
            _messages: &[Message],
            _tools: &[ToolSpec],
        ) -> Result<EventStream<'_>> {
            let mut guard = self.scripts.lock().unwrap();
            let events = if guard.is_empty() {
                vec![ProviderEvent::Done]
            } else {
                guard.remove(0)
            };
            let stream = futures::stream::iter(events);
            Ok(Box::pin(stream))
        }
    }
}
