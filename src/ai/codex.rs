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

pub struct CodexProvider {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub client: reqwest::Client,
    pub context: ProviderContext,
}

impl CodexProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        let entry = cfg
            .provider
            .codex
            .as_ref()
            .ok_or_else(|| AppError::provider("codex", "[provider.codex] missing"))?;
        let key_env = entry
            .api_key_env
            .clone()
            .unwrap_or_else(|| "OPENAI_API_KEY".to_string());
        let api_key_raw = std::env::var(&key_env);
        let context =
            ProviderContext::new(source, Some(key_env.clone()), api_key_raw.as_deref().ok());
        let api_key = api_key_raw.map_err(|_| {
            ProviderError::new("codex")
                .with_body(format!("env var {key_env} not set"))
                .with_context(&context)
                .into_app_error()
        })?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let model = entry.model.clone().unwrap_or_else(|| "gpt-4.1".to_string());
        // See ollama.rs — default 900s for streaming reasoning models.
        let client_timeout = entry.request_timeout_secs.unwrap_or(900);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(client_timeout))
            .build()?;
        Ok(Self {
            api_key,
            base_url,
            model,
            temperature: entry.temperature,
            client,
            context,
        })
    }
}

fn to_openai_messages(messages: &[Message]) -> Vec<Value> {
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
impl Provider for CodexProvider {
    fn name(&self) -> &'static str {
        "codex"
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
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<EventStream<'_>> {
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "messages": to_openai_messages(messages),
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(to_openai_tools(tools));
        }
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::codex", status = %status, body = %text, "provider HTTP error");
            let request_id = extract_request_id(&headers, &text);
            return Err(ProviderError::new("codex")
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
            let mut state = CodexParseState::default();
            let mut byte_stream = Box::pin(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes);
                        acc.push(&s);
                        for frame in acc.drain_frames() {
                            let outcome = handle_codex_frame(&frame, &mut state);
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

/// OpenAI Chat Completions のストリームパース中間状態。
#[derive(Default)]
pub(crate) struct CodexParseState {
    /// 現在組み立て中の tool_call: (id, name, partial JSON)
    pub current_tool: Option<(String, String, String)>,
}

pub(crate) struct CodexOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

fn flush_tool(state: &mut CodexParseState) -> Option<ProviderEvent> {
    let (id, name, args) = state.current_tool.take()?;
    let v: Value = serde_json::from_str(&args).unwrap_or(Value::Object(Default::default()));
    Some(ProviderEvent::ToolUse { id, name, args: v })
}

/// Chat Completions の SSE フレーム 1 件を解釈する純関数。
pub(crate) fn handle_codex_frame(frame: &str, state: &mut CodexParseState) -> CodexOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
        if let Some(ev) = flush_tool(state) {
            events.push(ev);
        }
        events.push(ProviderEvent::Done);
        return CodexOutcome { events, done: true };
    }
    let v: Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(_) => {
            return CodexOutcome {
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
    if let Some(arr) = delta.get("tool_calls").and_then(|c| c.as_array()) {
        for tc in arr {
            let id = tc.get("id").and_then(|s| s.as_str()).map(|s| s.to_string());
            let func = tc.get("function").cloned().unwrap_or(Value::Null);
            let name = func
                .get("name")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let args_part = func.get("arguments").and_then(|s| s.as_str()).unwrap_or("");
            match (id, name) {
                (Some(id), Some(name)) => {
                    state.current_tool = Some((id, name, args_part.to_string()));
                }
                (None, None) => {
                    if let Some((_, _, partial)) = state.current_tool.as_mut() {
                        partial.push_str(args_part);
                    }
                }
                (Some(id), None) => {
                    if let Some((existing, _, partial)) = state.current_tool.as_mut() {
                        if existing.is_empty() {
                            *existing = id;
                        }
                        partial.push_str(args_part);
                    }
                }
                (None, Some(name)) => {
                    if let Some((_, existing_name, partial)) = state.current_tool.as_mut() {
                        if existing_name.is_empty() {
                            *existing_name = name;
                        }
                        partial.push_str(args_part);
                    }
                }
            }
        }
    }
    if let Some(reason) = choice.get("finish_reason").and_then(|s| s.as_str()) {
        if reason == "tool_calls" {
            if let Some(ev) = flush_tool(state) {
                events.push(ev);
            }
        }
    }
    CodexOutcome {
        events,
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(frames: &[&str]) -> Vec<ProviderEvent> {
        let mut state = CodexParseState::default();
        let mut out = Vec::new();
        for f in frames {
            let outcome = handle_codex_frame(f, &mut state);
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
            r#"{"choices":[{"delta":{"content":"Hello"}}]}"#,
            r#"{"choices":[{"delta":{"content":", world"}}]}"#,
            "[DONE]",
        ]);
        let mut combined = String::new();
        let mut saw_done = false;
        for ev in evs {
            match ev {
                ProviderEvent::Text { delta } => combined.push_str(&delta),
                ProviderEvent::Done => saw_done = true,
                _ => {}
            }
        }
        assert_eq!(combined, "Hello, world");
        assert!(saw_done);
    }

    #[test]
    fn parses_streamed_tool_call_and_finish_reason() {
        let evs = collect(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"shell","arguments":"{\"cmd\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"function":{"arguments":"\"ls\"}"}}]}}]}"#,
            r#"{"choices":[{"finish_reason":"tool_calls"}]}"#,
            "[DONE]",
        ]);
        let tool = evs.iter().find_map(|e| {
            if let ProviderEvent::ToolUse { id, name, args } = e {
                Some((id.clone(), name.clone(), args.clone()))
            } else {
                None
            }
        });
        let (id, name, args) = tool.expect("tool_use missing");
        assert_eq!(id, "call_1");
        assert_eq!(name, "shell");
        assert_eq!(args.get("cmd").and_then(|v| v.as_str()), Some("ls"));
    }

    #[test]
    fn flushes_pending_tool_on_done_marker() {
        // finish_reason が来ずに [DONE] で締める変則ケースでも tool_use を出す
        let evs = collect(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_x","function":{"name":"fs_read","arguments":"{}"}}]}}]}"#,
            "[DONE]",
        ]);
        let has_tool = evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::ToolUse { name, .. } if name == "fs_read"));
        assert!(has_tool);
    }

    #[test]
    fn invalid_json_is_silently_skipped() {
        let mut state = CodexParseState::default();
        let outcome = handle_codex_frame("not-json", &mut state);
        assert!(outcome.events.is_empty());
        assert!(!outcome.done);
    }
}
