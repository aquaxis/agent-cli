use std::time::Duration;

use async_trait::async_trait;
use futures::stream::StreamExt;
use serde_json::{json, Value};

use crate::ai::stream::SseAccumulator;
use crate::ai::tool_bridge::to_openai_tools;
use crate::ai::{
    extract_request_id, Capabilities, EventStream, Message, Provider, ProviderContext,
    ProviderError, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

/// Provider that connects to a llama.cpp server (OpenAI-compatible /v1/chat/completions).
pub struct LlamaCppProvider {
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub client: reqwest::Client,
    pub api_key: Option<String>,
    pub context: ProviderContext,
}

impl LlamaCppProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        let entry =
            cfg.provider.llamacpp.as_ref().ok_or_else(|| {
                AppError::provider("llama.cpp", "[provider.\"llama.cpp\"] missing")
            })?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
        let model = entry.model.clone().unwrap_or_else(|| "default".to_string());
        let key_env = entry.api_key_env.clone();
        let api_key = key_env.as_ref().and_then(|k| std::env::var(k).ok());
        let context = ProviderContext::new(source, key_env, api_key.as_deref());
        // See ollama.rs — default 900s for streaming reasoning models.
        let client_timeout = entry.request_timeout_secs.unwrap_or(900);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(client_timeout))
            .build()?;
        Ok(Self {
            base_url,
            model,
            temperature: entry.temperature,
            client,
            api_key,
            context,
        })
    }
}

fn to_oai_messages(messages: &[Message]) -> Vec<Value> {
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
impl Provider for LlamaCppProvider {
    fn name(&self) -> &'static str {
        "llama.cpp"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: true,
            // tools depend on the server build; we advertise true and let runtime degrade gracefully on 4xx errors.
            tool_use: true,
            thinking: false,
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
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "messages": to_oai_messages(messages),
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(to_openai_tools(tools));
        }
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }

        let url = format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        );
        let mut req = self
            .client
            .post(url)
            .header("content-type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::llamacpp", status = %status, body = %text, "provider HTTP error");
            let request_id = extract_request_id(&headers, &text);
            return Err(ProviderError::new("llama.cpp")
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

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut acc = SseAccumulator::new();
            let mut byte_stream = Box::pin(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes);
                        acc.push(&s);
                        for frame in acc.drain_frames() {
                            let outcome = handle_llamacpp_frame(&frame);
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

pub(crate) struct LlamaCppOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

/// Pure function to interpret a single SSE frame from llama.cpp (OpenAI-compatible).
pub(crate) fn handle_llamacpp_frame(frame: &str) -> LlamaCppOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
        events.push(ProviderEvent::Done);
        return LlamaCppOutcome { events, done: true };
    }
    let v: Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(_) => {
            return LlamaCppOutcome {
                events,
                done: false,
            }
        }
    };
    let choice = v
        .get("choices")
        .and_then(|c| c.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let delta = choice.get("delta").cloned().unwrap_or(Value::Null);
    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
        if !text.is_empty() {
            events.push(ProviderEvent::Text {
                delta: text.to_string(),
            });
        }
    }
    LlamaCppOutcome {
        events,
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(frames: &[&str]) -> Vec<ProviderEvent> {
        let mut out = Vec::new();
        for f in frames {
            let outcome = handle_llamacpp_frame(f);
            out.extend(outcome.events);
            if outcome.done {
                break;
            }
        }
        out
    }

    #[test]
    fn parses_text_chunks_then_done() {
        let evs = collect(&[
            r#"{"choices":[{"delta":{"content":"hello"}}]}"#,
            r#"{"choices":[{"delta":{"content":" world"}}]}"#,
            "[DONE]",
        ]);
        let mut text = String::new();
        let mut saw_done = false;
        for ev in evs {
            match ev {
                ProviderEvent::Text { delta } => text.push_str(&delta),
                ProviderEvent::Done => saw_done = true,
                _ => {}
            }
        }
        assert_eq!(text, "hello world");
        assert!(saw_done);
    }

    #[test]
    fn empty_content_chunks_are_ignored() {
        let evs = collect(&[
            r#"{"choices":[{"delta":{"content":""}}]}"#,
            r#"{"choices":[{"delta":{"content":"ok"}}]}"#,
        ]);
        let texts: Vec<_> = evs
            .iter()
            .filter_map(|e| {
                if let ProviderEvent::Text { delta } = e {
                    Some(delta.clone())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(texts, vec!["ok".to_string()]);
    }

    #[test]
    fn invalid_json_is_silently_skipped() {
        let outcome = handle_llamacpp_frame("not-json");
        assert!(outcome.events.is_empty());
        assert!(!outcome.done);
    }
}
