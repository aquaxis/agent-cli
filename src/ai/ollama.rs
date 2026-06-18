use std::time::Duration;

use async_trait::async_trait;
use futures::stream::StreamExt;
use serde_json::{json, Value};

use crate::ai::tool_bridge::to_ollama_tools;
use crate::ai::{
    extract_request_id, Capabilities, EventStream, Message, Provider, ProviderContext,
    ProviderError, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

/// Default number of retries for transient errors when not configured.
const DEFAULT_MAX_RETRIES: u32 = 3;

pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    /// How many times to retry a transient error before surfacing it (FR-13).
    pub max_retries: u32,
    pub client: reqwest::Client,
    pub context: ProviderContext,
}

impl OllamaProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        let entry = cfg
            .provider
            .ollama
            .as_ref()
            .ok_or_else(|| AppError::provider("ollama", "[provider.ollama] missing"))?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
        let model = entry
            .model
            .clone()
            .unwrap_or_else(|| "glm-5.1:cloud".to_string());
        // Default 900s (15 min) — cloud reasoning models (e.g. glm-5.1:cloud)
        // can stream `thinking` tokens for several minutes before producing
        // actual content, and reqwest's `timeout()` applies to the entire
        // streaming response, not per-chunk. Override via
        // `[provider.ollama] request_timeout_secs = N` in agent-cli config.
        let client_timeout = entry.request_timeout_secs.unwrap_or(900);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(client_timeout))
            .build()?;
        // Ollama generally does not require an API key (local). The entry.api_key_env
        // is set only when needed for cloud routes, so key info is reflected in the diagnostic context.
        let key_env = entry.api_key_env.clone();
        let key_value = key_env.as_ref().and_then(|k| std::env::var(k).ok());
        let context = ProviderContext::new(source, key_env, key_value.as_deref());
        let max_retries = entry.max_retries.map(|n| n as u32).unwrap_or(DEFAULT_MAX_RETRIES);
        Ok(Self {
            base_url,
            model,
            temperature: entry.temperature,
            max_retries,
            client,
            context,
        })
    }
}

/// Build the `/api/chat` request body. Pure so the request shape (notably the
/// `model` name, which must keep any `:cloud` tag verbatim — FR-12) is testable
/// without a network call.
fn build_chat_body(
    model: &str,
    messages: &[Message],
    tools: &[ToolSpec],
    temperature: Option<f32>,
) -> Value {
    let mut body = json!({
        "model": model,
        "stream": true,
        "messages": to_ollama_messages(messages),
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(to_ollama_tools(tools));
    }
    if let Some(t) = temperature {
        body["options"] = json!({ "temperature": t });
    }
    body
}

/// Classify an Ollama HTTP failure as transient (worth retrying). Transient =
/// HTTP 503/429/500/502/504, or a body indicating the model is temporarily
/// unavailable ("overloaded" / "temporarily" / "please retry" / "try again").
/// Non-transient failures (e.g. 400/404 model-not-found) are not retried (FR-13).
fn is_transient(status: u16, body: &str) -> bool {
    if matches!(status, 503 | 429 | 500 | 502 | 504) {
        return true;
    }
    let b = body.to_ascii_lowercase();
    b.contains("overloaded")
        || b.contains("temporarily")
        || b.contains("please retry")
        || b.contains("try again")
}

/// Exponential backoff in milliseconds for retry `attempt` (0-based): ~1s, 2s,
/// 4s, … capped at 8s. Pure and deterministic so it is unit-testable (NFR-05).
fn backoff_delay_ms(attempt: u32) -> u64 {
    let ms = 1000u64.saturating_mul(1u64 << attempt.min(3));
    ms.min(8000)
}

fn to_ollama_messages(messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::new();
    for m in messages {
        match m {
            Message::System { content } => {
                out.push(json!({"role": "system", "content": content}));
            }
            Message::User { content } => {
                out.push(json!({"role": "user", "content": content}));
            }
            // Not reachable from the shipped `kind="opencode"` config; tool
            // calls are not serialized here (latent — deferred).
            Message::Assistant { content, .. } => {
                out.push(json!({"role": "assistant", "content": content}));
            }
            Message::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                }));
            }
        }
    }
    out
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: true,
            tool_use: true,
            // FR-03-1-2 / design doc 4.3C: Thinking-capable models (e.g. glm-5.1:cloud)
            // return `message.thinking`, so this is true. Non-capable models simply
            // emit nothing from parse_ndjson_line, which is harmless (policy A).
            thinking: true,
        }
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<EventStream<'_>> {
        let body = build_chat_body(&self.model, messages, tools, self.temperature);
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        // Establish the request, retrying transient errors (e.g. cloud cold-start
        // / "temporarily overloaded" 503s) with bounded exponential backoff before
        // surfacing the error. `ollama run` tolerates these; we mirror that (FR-13).
        // Only the initial request/status is retried — not a stream already begun.
        let mut attempt: u32 = 0;
        let resp = loop {
            let send_result = self
                .client
                .post(url.as_str())
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await;

            match send_result {
                Ok(resp) if resp.status().is_success() => break resp,
                Ok(resp) => {
                    let status = resp.status();
                    let headers = resp.headers().clone();
                    let text = resp.text().await.unwrap_or_default();
                    tracing::debug!(target: "agent_cli::ai::ollama", status = %status, body = %text, "provider HTTP error");
                    if attempt < self.max_retries && is_transient(status.as_u16(), &text) {
                        let delay = backoff_delay_ms(attempt);
                        tracing::warn!(
                            target: "agent_cli::ai::ollama",
                            attempt = attempt + 1,
                            max_retries = self.max_retries,
                            status = %status,
                            delay_ms = delay,
                            "transient ollama error; retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt += 1;
                        continue;
                    }
                    let request_id = extract_request_id(&headers, &text);
                    return Err(ProviderError::new("ollama")
                        .with_http(
                            status.as_u16(),
                            status.canonical_reason().unwrap_or("").to_string(),
                        )
                        .with_body(text)
                        .with_request_id(request_id)
                        .with_context(&self.context)
                        .detect_hint()
                        .into_app_error());
                }
                Err(e) => {
                    // Network-level failure: retry connect/timeout errors, which
                    // are also transient for a waking cloud route.
                    if attempt < self.max_retries && (e.is_timeout() || e.is_connect()) {
                        let delay = backoff_delay_ms(attempt);
                        tracing::warn!(
                            target: "agent_cli::ai::ollama",
                            attempt = attempt + 1,
                            max_retries = self.max_retries,
                            delay_ms = delay,
                            error = %e,
                            "transient ollama network error; retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        };

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut buffer = String::new();
            let mut byte_stream = Box::pin(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&s);
                        while let Some(idx) = buffer.find('\n') {
                            let line: String = buffer.drain(..=idx).collect();
                            let outcome = parse_ndjson_line(&line);
                            for ev in outcome.events {
                                yield ev;
                            }
                            if outcome.done {
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        yield ProviderEvent::Error { message: e.to_string() };
                        return;
                    }
                }
            }
            yield ProviderEvent::Done;
        };
        Ok(Box::pin(stream))
    }
}

pub(crate) struct OllamaLineOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

/// Pure function to interpret a single NDJSON line from Ollama `/api/chat`.
pub(crate) fn parse_ndjson_line(line: &str) -> OllamaLineOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return OllamaLineOutcome {
            events: Vec::new(),
            done: false,
        };
    }
    let v: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => {
            return OllamaLineOutcome {
                events: Vec::new(),
                done: false,
            }
        }
    };
    let mut events = Vec::new();
    if let Some(msg) = v.get("message") {
        // FR-03-1-2 / design doc 4.3C: Emission order is Thinking -> Text -> ToolUse (consistent with Anthropic spec).
        if let Some(thinking) = msg.get("thinking").and_then(|t| t.as_str()) {
            if !thinking.is_empty() {
                events.push(ProviderEvent::Thinking {
                    text: thinking.to_string(),
                });
            }
        }
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            if !content.is_empty() {
                events.push(ProviderEvent::Text {
                    delta: content.to_string(),
                });
            }
        }
        if let Some(arr) = msg.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in arr {
                let id = tc
                    .get("id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let func = tc.get("function").cloned().unwrap_or(Value::Null);
                let name = func
                    .get("name")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = func
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                events.push(ProviderEvent::ToolUse { id, name, args });
            }
        }
    }
    let done = v.get("done").and_then(|b| b.as_bool()).unwrap_or(false);
    if done {
        events.push(ProviderEvent::Done);
    }
    OllamaLineOutcome { events, done }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(lines: &[&str]) -> Vec<ProviderEvent> {
        let mut out = Vec::new();
        for l in lines {
            let outcome = parse_ndjson_line(l);
            out.extend(outcome.events);
            if outcome.done {
                break;
            }
        }
        out
    }

    #[test]
    fn parses_text_chunks_and_done() {
        let evs = collect(&[
            r#"{"message":{"role":"assistant","content":"Hi"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":" there"},"done":false}"#,
            r#"{"done":true}"#,
        ]);
        let mut text = String::new();
        let mut done_count = 0;
        for ev in evs {
            match ev {
                ProviderEvent::Text { delta } => text.push_str(&delta),
                ProviderEvent::Done => done_count += 1,
                _ => {}
            }
        }
        assert_eq!(text, "Hi there");
        assert_eq!(done_count, 1);
    }

    #[test]
    fn parses_tool_calls() {
        let line = r#"{"message":{"role":"assistant","content":"","tool_calls":[{"id":"t1","function":{"name":"shell","arguments":{"cmd":"ls"}}}]},"done":false}"#;
        let outcome = parse_ndjson_line(line);
        assert!(!outcome.done);
        let tool = outcome.events.iter().find_map(|e| {
            if let ProviderEvent::ToolUse { id, name, args } = e {
                Some((id.clone(), name.clone(), args.clone()))
            } else {
                None
            }
        });
        let (id, name, args) = tool.expect("tool_use missing");
        assert_eq!(id, "t1");
        assert_eq!(name, "shell");
        assert_eq!(args.get("cmd").and_then(|v| v.as_str()), Some("ls"));
    }

    #[test]
    fn empty_line_returns_no_events() {
        let outcome = parse_ndjson_line("");
        assert!(outcome.events.is_empty());
        assert!(!outcome.done);
    }

    #[test]
    fn invalid_json_is_silently_skipped() {
        let outcome = parse_ndjson_line("not-json");
        assert!(outcome.events.is_empty());
        assert!(!outcome.done);
    }

    /// FR-03-1-2 / design doc 4.3C: Emit `message.thinking` as `ProviderEvent::Thinking`.
    /// When both thinking and content exist in the same frame, emit in Thinking -> Text order.
    #[test]
    fn parses_thinking_field_emits_thinking_event() {
        let line = r#"{"message":{"role":"assistant","thinking":"reason about the prompt","content":"hello"},"done":false}"#;
        let outcome = parse_ndjson_line(line);
        assert!(!outcome.done);
        assert_eq!(
            outcome.events.len(),
            2,
            "expected Thinking + Text, got: {:?}",
            outcome.events
        );
        match &outcome.events[0] {
            ProviderEvent::Thinking { text } => {
                assert_eq!(text, "reason about the prompt");
            }
            other => panic!("first event should be Thinking, got: {other:?}"),
        }
        match &outcome.events[1] {
            ProviderEvent::Text { delta } => {
                assert_eq!(delta, "hello");
            }
            other => panic!("second event should be Text, got: {other:?}"),
        }
    }

    /// Empty `message.thinking` is not emitted (same policy as the existing content empty-string skip).
    #[test]
    fn empty_thinking_is_not_emitted() {
        let line = r#"{"message":{"role":"assistant","thinking":"","content":"x"},"done":false}"#;
        let outcome = parse_ndjson_line(line);
        let thinking_count = outcome
            .events
            .iter()
            .filter(|e| matches!(e, ProviderEvent::Thinking { .. }))
            .count();
        assert_eq!(thinking_count, 0, "empty thinking should not emit");
        let text_count = outcome
            .events
            .iter()
            .filter(|e| matches!(e, ProviderEvent::Text { .. }))
            .count();
        assert_eq!(text_count, 1, "non-empty content should still emit Text");
    }

    /// A thinking-only frame (no content or tool_calls) still emits a Thinking event.
    /// Covers the case where `glm-5.1:cloud` streams a long thinking phase before the response.
    #[test]
    fn thinking_only_frame_emits_only_thinking() {
        let line = r#"{"message":{"role":"assistant","thinking":"step 1: ..."},"done":false}"#;
        let outcome = parse_ndjson_line(line);
        assert_eq!(outcome.events.len(), 1);
        assert!(matches!(
            &outcome.events[0],
            ProviderEvent::Thinking { text } if text == "step 1: ..."
        ));
    }

    // --- Request body: model name preserved verbatim (FR-12 / Task 10) ---

    #[test]
    fn build_chat_body_preserves_cloud_tag() {
        let msgs = vec![Message::User {
            content: "hi".into(),
        }];
        let body = build_chat_body("glm-5.2:cloud", &msgs, &[], None);
        assert_eq!(
            body.get("model").and_then(|v| v.as_str()),
            Some("glm-5.2:cloud"),
            "the :cloud tag must be sent verbatim"
        );
        assert_eq!(body.get("stream").and_then(|v| v.as_bool()), Some(true));
        // No tools provided -> no tools key.
        assert!(body.get("tools").is_none());
        // No temperature -> no options key.
        assert!(body.get("options").is_none());
    }

    #[test]
    fn build_chat_body_includes_temperature_when_set() {
        let msgs = vec![Message::User {
            content: "hi".into(),
        }];
        let body = build_chat_body("glm-5.2:cloud", &msgs, &[], Some(0.5));
        assert!(body.get("options").is_some());
    }

    // --- Transient-error classification (FR-13 / Task 12) ---

    #[test]
    fn is_transient_503_overloaded_is_true() {
        assert!(is_transient(
            503,
            r#"{"error":"model 'glm-5.2' is temporarily overloaded, please retry shortly"}"#
        ));
    }

    #[test]
    fn is_transient_503_generic_is_true() {
        assert!(is_transient(503, "service unavailable"));
    }

    #[test]
    fn is_transient_retryable_statuses() {
        assert!(is_transient(429, ""));
        assert!(is_transient(500, ""));
        assert!(is_transient(502, ""));
        assert!(is_transient(504, ""));
    }

    #[test]
    fn is_transient_400_404_is_false() {
        assert!(!is_transient(400, "bad request"));
        assert!(!is_transient(404, r#"{"error":"model not found"}"#));
    }

    #[test]
    fn is_transient_matches_body_phrases_case_insensitive() {
        assert!(is_transient(400, "Please Retry shortly"));
        assert!(is_transient(403, "the model is OVERLOADED"));
        assert!(!is_transient(400, "permanent configuration error"));
    }

    // --- Backoff schedule is bounded (NFR-05 / Task 12) ---

    #[test]
    fn backoff_delay_is_exponential_and_capped() {
        assert_eq!(backoff_delay_ms(0), 1000);
        assert_eq!(backoff_delay_ms(1), 2000);
        assert_eq!(backoff_delay_ms(2), 4000);
        assert_eq!(backoff_delay_ms(3), 8000);
        // Capped at 8s for all higher attempts; never overflows.
        assert_eq!(backoff_delay_ms(4), 8000);
        assert_eq!(backoff_delay_ms(10), 8000);
        assert_eq!(backoff_delay_ms(64), 8000);
    }
}
