//! OpenAI Chat Completions wire helpers shared by the OpenAI-compatible
//! backends (`codex`/`strata`, `opencode` cloud, `llama.cpp`): the plain
//! message mapping and the streaming (`[DONE]`-terminated SSE) frame parser.

use serde_json::{json, Value};

use crate::ai::stream::FrameOutcome;
use crate::ai::{Message, ProviderEvent};

/// Plain OpenAI chat-completions message mapping. Assistant tool calls are
/// not serialized here (latent — not reachable from the shipped config);
/// opencode's cloud path has its own full mapping.
pub(crate) fn to_plain_openai_messages(messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::new();
    for m in messages {
        match m {
            Message::System { content } => {
                out.push(json!({"role": "system", "content": content}));
            }
            Message::User { content } => {
                out.push(json!({"role": "user", "content": content}));
            }
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

/// Intermediate state for OpenAI Chat Completions stream parsing.
#[derive(Default)]
pub(crate) struct OpenAiParseState {
    /// Currently assembling tool_call: (id, name, partial JSON args).
    pub current_tool: Option<(String, String, String)>,
}

fn flush_tool(state: &mut OpenAiParseState) -> Option<ProviderEvent> {
    let (id, name, args) = state.current_tool.take()?;
    let v: Value = serde_json::from_str(&args).unwrap_or(Value::Object(Default::default()));
    Some(ProviderEvent::ToolUse { id, name, args: v })
}

/// Pure parser for one chat-completions SSE frame: text deltas, incrementally
/// assembled `tool_calls`, and a `[DONE]` terminator. Invalid JSON is
/// silently skipped. With `reasoning`, `delta.reasoning_content` (DeepSeek
/// thinking mode) is surfaced as `Thinking` so process_turn can accumulate
/// and echo it back — the endpoint rejects the next request if the prior
/// assistant turn omits it.
pub(crate) fn handle_openai_frame(
    frame: &str,
    state: &mut OpenAiParseState,
    reasoning: bool,
) -> FrameOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
        if let Some(ev) = flush_tool(state) {
            events.push(ev);
        }
        events.push(ProviderEvent::Done);
        return FrameOutcome { events, done: true };
    }
    let v: Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(_) => {
            return FrameOutcome {
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
    if reasoning {
        if let Some(r) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
            if !r.is_empty() {
                events.push(ProviderEvent::Thinking {
                    text: r.to_string(),
                });
            }
        }
    }
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
    FrameOutcome {
        events,
        done: false,
    }
}
