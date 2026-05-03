use std::time::Duration;

use async_trait::async_trait;
use futures::stream::StreamExt;
use serde_json::{json, Value};

use crate::ai::stream::SseAccumulator;
use crate::ai::tool_bridge::to_anthropic_tools;
use crate::ai::{
    extract_request_id, Capabilities, EventStream, Message, Provider, ProviderContext,
    ProviderError, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

pub struct ClaudeProvider {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub thinking: bool,
    pub temperature: Option<f32>,
    pub client: reqwest::Client,
    pub context: ProviderContext,
}

impl ClaudeProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        let entry = cfg
            .provider
            .claude
            .as_ref()
            .ok_or_else(|| AppError::provider("claude", "[provider.claude] missing"))?;
        let key_env = entry
            .api_key_env
            .clone()
            .unwrap_or_else(|| "ANTHROPIC_API_KEY".to_string());
        let api_key_raw = std::env::var(&key_env);
        let context =
            ProviderContext::new(source, Some(key_env.clone()), api_key_raw.as_deref().ok());
        let api_key = api_key_raw.map_err(|_| {
            ProviderError::new("claude")
                .with_body(format!("env var {key_env} not set"))
                .with_context(&context)
                .into_app_error()
        })?;
        let base_url = entry
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());
        let model = entry
            .model
            .clone()
            .unwrap_or_else(|| "claude-opus-4-7".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;
        Ok(Self {
            api_key,
            base_url,
            model,
            thinking: entry.thinking.unwrap_or(true),
            temperature: entry.temperature,
            client,
            context,
        })
    }
}

fn to_anthropic_messages(messages: &[Message]) -> (Option<String>, Vec<Value>) {
    let mut system: Option<String> = None;
    let mut out: Vec<Value> = Vec::new();
    for m in messages {
        match m {
            Message::System { content } => {
                if let Some(s) = system.as_mut() {
                    s.push('\n');
                    s.push_str(content);
                } else {
                    system = Some(content.clone());
                }
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
                is_error,
            } => {
                out.push(json!({
                    "role": "user",
                    "content": [{
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "is_error": *is_error,
                        "content": content,
                    }],
                }));
            }
        }
    }
    (system, out)
}

#[async_trait]
impl Provider for ClaudeProvider {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: true,
            tool_use: true,
            thinking: self.thinking,
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
        let (system, msgs) = to_anthropic_messages(messages);
        let mut body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "stream": true,
            "messages": msgs,
        });
        if let Some(sys) = system {
            body["system"] = Value::String(sys);
        }
        if !tools.is_empty() {
            body["tools"] = Value::Array(to_anthropic_tools(tools));
        }
        if self.thinking {
            body["thinking"] = json!({"type": "enabled", "budget_tokens": 1024});
        }
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }

        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::claude", status = %status, body = %text, "provider HTTP error");
            let request_id = extract_request_id(&headers, &text);
            return Err(ProviderError::new("claude")
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
            let mut state = ClaudeParseState::default();
            let mut byte_stream = Box::pin(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes);
                        acc.push(&s);
                        for frame in acc.drain_frames() {
                            let outcome = handle_frame(&frame, &mut state);
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

/// Claude SSE フレーム処理の中間状態。
#[derive(Default)]
pub(crate) struct ClaudeParseState {
    pub current_tool: Option<(String, String, String)>, // (id, name, partial JSON)
}

pub(crate) struct ParseOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

/// 単一フレーム文字列を解釈し、イベント列と終了フラグを返す。`drain_frames` 後の各要素に適用する。
pub(crate) fn handle_frame(frame: &str, state: &mut ClaudeParseState) -> ParseOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
        events.push(ProviderEvent::Done);
        return ParseOutcome { events, done: true };
    }
    let v: Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(e) => {
            events.push(ProviderEvent::Error {
                message: format!("parse error: {e}"),
            });
            return ParseOutcome {
                events,
                done: false,
            };
        }
    };
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match ty {
        "content_block_start" => {
            if let Some(block) = v.get("content_block") {
                let bt = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if bt == "tool_use" {
                    let id = block
                        .get("id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    state.current_tool = Some((id, name, String::new()));
                }
            }
        }
        "content_block_delta" => {
            let delta = v.get("delta").cloned().unwrap_or(Value::Null);
            let dt = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match dt {
                "text_delta" => {
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        events.push(ProviderEvent::Text {
                            delta: text.to_string(),
                        });
                    }
                }
                "thinking_delta" => {
                    if let Some(text) = delta.get("thinking").and_then(|t| t.as_str()) {
                        events.push(ProviderEvent::Thinking {
                            text: text.to_string(),
                        });
                    }
                }
                "input_json_delta" => {
                    if let Some((_, _, partial)) = state.current_tool.as_mut() {
                        if let Some(p) = delta.get("partial_json").and_then(|t| t.as_str()) {
                            partial.push_str(p);
                        }
                    }
                }
                _ => {}
            }
        }
        "content_block_stop" => {
            if let Some((id, name, partial)) = state.current_tool.take() {
                let args: Value =
                    serde_json::from_str(&partial).unwrap_or(Value::Object(Default::default()));
                events.push(ProviderEvent::ToolUse { id, name, args });
            }
        }
        "message_stop" => {
            events.push(ProviderEvent::Done);
            return ParseOutcome { events, done: true };
        }
        "error" => {
            let msg = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error")
                .to_string();
            events.push(ProviderEvent::Error { message: msg });
        }
        _ => {}
    }
    ParseOutcome {
        events,
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_frames(frames: &[&str]) -> Vec<ProviderEvent> {
        let mut state = ClaudeParseState::default();
        let mut out = Vec::new();
        for f in frames {
            let outcome = handle_frame(f, &mut state);
            out.extend(outcome.events);
            if outcome.done {
                break;
            }
        }
        out
    }

    #[test]
    fn parses_text_delta() {
        let evs = run_frames(&[
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#,
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":", world"}}"#,
            r#"{"type":"message_stop"}"#,
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
    fn parses_thinking_delta() {
        let evs = run_frames(&[
            r#"{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"考え中"}}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        let has_thinking = evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::Thinking { text } if text == "考え中"));
        assert!(has_thinking);
    }

    #[test]
    fn parses_tool_use_block() {
        let evs = run_frames(&[
            r#"{"type":"content_block_start","content_block":{"type":"tool_use","id":"toolu_1","name":"shell"}}"#,
            r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{\"cmd\":\"ls"}}"#,
            r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"\"}"}}"#,
            r#"{"type":"content_block_stop"}"#,
            r#"{"type":"message_stop"}"#,
        ]);
        let tool = evs.iter().find_map(|e| {
            if let ProviderEvent::ToolUse { id, name, args } = e {
                Some((id.clone(), name.clone(), args.clone()))
            } else {
                None
            }
        });
        let (id, name, args) = tool.expect("tool_use missing");
        assert_eq!(id, "toolu_1");
        assert_eq!(name, "shell");
        assert_eq!(args.get("cmd").and_then(|v| v.as_str()), Some("ls"));
    }

    #[test]
    fn handles_error_frame() {
        let evs = run_frames(&[r#"{"type":"error","error":{"message":"boom"}}"#]);
        let has_err = evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::Error { message } if message == "boom"));
        assert!(has_err);
    }
}
