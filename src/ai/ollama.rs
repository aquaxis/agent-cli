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

pub struct OllamaProvider {
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
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
        // Ollama は基本的に API キー不要（ローカル）。クラウド経路で必要な場合のみ
        // entry.api_key_env が設定される運用なので、診断コンテキストには key 情報を反映する。
        let key_env = entry.api_key_env.clone();
        let key_value = key_env.as_ref().and_then(|k| std::env::var(k).ok());
        let context = ProviderContext::new(source, key_env, key_value.as_deref());
        Ok(Self {
            base_url,
            model,
            temperature: entry.temperature,
            client,
            context,
        })
    }
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
            Message::Assistant { content } => {
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
            // FR-03-1-2 / 設計書 4.3C：thinking 対応モデル（例：glm-5.1:cloud）が
            // `message.thinking` を返すため true。非対応モデルでは parse_ndjson_line が
            // 何も emit しないだけで害がない（方針 A）。
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
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "messages": to_ollama_messages(messages),
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(to_ollama_tools(tools));
        }
        if let Some(t) = self.temperature {
            body["options"] = json!({ "temperature": t });
        }
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::ollama", status = %status, body = %text, "provider HTTP error");
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

/// Ollama `/api/chat` の NDJSON 行 1 件を解釈する純関数。
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
        // FR-03-1-2 / 設計書 4.3C：emit 順は Thinking → Text → ToolUse（Anthropic 仕様と整合）。
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

    /// FR-03-1-2 / 設計書 4.3C：`message.thinking` を `ProviderEvent::Thinking` として emit。
    /// 同一フレーム内に thinking と content がある場合は Thinking → Text の順で発行する。
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

    /// 空の `message.thinking` は emit しない（既存の content 空文字スキップと同方針）。
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

    /// thinking のみのフレーム（content・tool_calls なし）でも Thinking が emit される。
    /// `glm-5.1:cloud` が回答前に thinking ストリームだけを長く流すケースを想定。
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
}
