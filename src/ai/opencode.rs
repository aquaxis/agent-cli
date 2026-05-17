//! OpenCode provider.
//!
//! Supports two modes selected by configuration (see AI_PRJ_DESIGN.md 3.1):
//!
//! * **Cloud (OpenCode Zen)** — when an API key is resolved. Two wire formats
//!   selected by `[provider.opencode] api`: `"openai"` (default) →
//!   `POST {base_url}/chat/completions` (`[DONE]` SSE); `"anthropic"` →
//!   `POST {base_url}/messages` (Anthropic SSE, reuses the Claude parser).
//!   `Authorization: Bearer <key>` either way. Default cloud `base_url` is
//!   `https://opencode.ai/zen/v1` (use `https://opencode.ai/zen/go/v1` for
//!   the "go" endpoints).
//! * **Local (`opencode serve`)** — when no API key is resolved. Performs the
//!   native session handshake: `POST /session` then `POST /session/:id/message`
//!   which returns a synchronous JSON body `{ info, parts }`. No auth header.
//!   Default local `base_url` is `http://127.0.0.1:4096`.
//!
//! All wire-protocol specifics are isolated in `to_openai_messages`,
//! `flatten_history`, `handle_opencode_frame` (cloud SSE) and
//! `parse_local_parts` (local sync JSON) so the `Provider` trait and the agent
//! loop are unaffected by either shape.

use std::time::Duration;

use async_trait::async_trait;
use futures::stream::StreamExt;
use serde_json::{json, Value};

use crate::ai::claude::{handle_frame, to_anthropic_messages, ClaudeParseState};
use crate::ai::stream::SseAccumulator;
use crate::ai::tool_bridge::{to_anthropic_tools, to_openai_tools};
use crate::ai::{
    extract_request_id, Capabilities, EventStream, Message, Provider, ProviderContext,
    ProviderError, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

/// Cloud-mode wire format / endpoint (opencode Zen). Selected by
/// `[provider.opencode] api`; ignored in local mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloudApi {
    /// OpenAI-compatible `{base}/chat/completions` (default).
    OpenAi,
    /// Anthropic-compatible `{base}/messages`.
    Anthropic,
}

impl CloudApi {
    fn parse(s: Option<&str>) -> Self {
        match s.map(|x| x.trim().to_ascii_lowercase()).as_deref() {
            Some("anthropic") | Some("messages") => CloudApi::Anthropic,
            _ => CloudApi::OpenAi,
        }
    }

    fn path(self) -> &'static str {
        match self {
            CloudApi::OpenAi => "/chat/completions",
            CloudApi::Anthropic => "/messages",
        }
    }
}

/// Cloud endpoint URL for `base` under the selected API style.
fn cloud_url(base: &str, api: CloudApi) -> String {
    format!("{}{}", base.trim_end_matches('/'), api.path())
}

/// Cached local-mode session state (opt-in `persistent_session`).
#[derive(Default)]
struct PersistState {
    /// OpenCode server `session_id`, or `None` until first created.
    id: Option<String>,
    /// Count of agent-cli history messages already delivered to the session.
    sent_count: usize,
    /// System prompt the session was created with (reset on change).
    sys: String,
}

pub struct OpenCodeProvider {
    /// `Some` => cloud (Zen) mode; `None` => local `opencode serve` mode.
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    /// Opt-in: reuse one server session across turns (local mode only).
    pub persistent_session: bool,
    /// Cloud wire format (OpenAI vs Anthropic compatible). Local mode ignores.
    pub(crate) cloud_api: CloudApi,
    pub client: reqwest::Client,
    pub context: ProviderContext,
    /// Persistent-session bookkeeping. `tokio::sync::Mutex` because it is
    /// held across `.await`; calls are serialized (turns are sequential).
    session: tokio::sync::Mutex<PersistState>,
}

impl OpenCodeProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        let entry = cfg
            .provider
            .opencode
            .as_ref()
            .ok_or_else(|| AppError::provider("opencode", "[provider.opencode] missing"))?;
        // `api_key_env` is NOT defaulted (design 3.1). A missing key simply
        // means local mode; it is only an error if a cloud endpoint rejects
        // the request, surfaced naturally as an HTTP 401 ProviderError.
        let key_env = entry.api_key_env.clone();
        let key_value = key_env.as_ref().and_then(|k| std::env::var(k).ok());
        let context = ProviderContext::new(source, key_env, key_value.as_deref());
        let base_url = entry.base_url.clone().unwrap_or_else(|| {
            if key_value.is_some() {
                "https://opencode.ai/zen/v1".to_string()
            } else {
                "http://127.0.0.1:4096".to_string()
            }
        });
        let model = entry
            .model
            .clone()
            .unwrap_or_else(|| "claude-sonnet-4-5".to_string());
        // See ollama.rs — default 900s for streaming reasoning models.
        let client_timeout = entry.request_timeout_secs.unwrap_or(900);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(client_timeout))
            .build()?;
        Ok(Self {
            api_key: key_value,
            base_url,
            model,
            temperature: entry.temperature,
            persistent_session: entry.persistent_session.unwrap_or(false),
            cloud_api: CloudApi::parse(entry.api.as_deref()),
            client,
            context,
            session: tokio::sync::Mutex::new(PersistState::default()),
        })
    }

    fn provider_error(&self, status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap, text: String) -> AppError {
        let request_id = extract_request_id(headers, &text);
        ProviderError::new("opencode")
            .with_http(
                status.as_u16(),
                status.canonical_reason().unwrap_or("").to_string(),
            )
            .with_body(text)
            .with_request_id(request_id)
            .with_context(&self.context)
            .detect_hint()
            .into_app_error()
    }

    /// Cloud (OpenCode Zen) dispatcher — picks the OpenAI- or
    /// Anthropic-compatible endpoint per `[provider.opencode] api`.
    async fn complete_stream_cloud(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<EventStream<'_>> {
        match self.cloud_api {
            CloudApi::OpenAi => self.complete_stream_cloud_openai(messages, tools).await,
            CloudApi::Anthropic => self.complete_stream_cloud_anthropic(messages, tools).await,
        }
    }

    /// Cloud — OpenAI-compatible streaming chat completions
    /// (`{base}/chat/completions`, `[DONE]`-terminated SSE).
    async fn complete_stream_cloud_openai(
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
        let url = cloud_url(&self.base_url, CloudApi::OpenAi);
        let mut req = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::opencode", status = %status, body = %text, "provider HTTP error");
            return Err(self.provider_error(status, &headers, text));
        }

        let byte_stream = resp.bytes_stream();
        let stream = async_stream::stream! {
            let mut acc = SseAccumulator::new();
            let mut state = OpenCodeParseState::default();
            let mut byte_stream = Box::pin(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let s = String::from_utf8_lossy(&bytes);
                        acc.push(&s);
                        for frame in acc.drain_frames() {
                            let outcome = handle_opencode_frame(&frame, &mut state);
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

    /// Cloud — Anthropic-compatible streaming messages
    /// (`{base}/messages`). Reuses the Claude request/SSE machinery; auth is
    /// sent as both `Authorization: Bearer` (Zen convention) and `x-api-key`
    /// (native Anthropic), plus `anthropic-version`, for gateway compatibility.
    async fn complete_stream_cloud_anthropic(
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
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        let url = cloud_url(&self.base_url, CloudApi::Anthropic);
        let mut req = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key).header("x-api-key", key);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::opencode", status = %status, body = %text, "provider HTTP error");
            return Err(self.provider_error(status, &headers, text));
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

    /// Local (`opencode serve`) — create an ephemeral session then post one
    /// message; the response is synchronous JSON `{ info, parts }`.
    ///
    /// agent-cli replays full history every turn, so the entire conversation
    /// is flattened into a single message into a fresh session (design 3.4
    /// risk mitigation: ephemeral session per turn).
    async fn complete_stream_local(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let base = self.base_url.trim_end_matches('/');

        // 1. Create a session.
        let resp = self
            .client
            .post(format!("{base}/session"))
            .header("content-type", "application/json")
            .json(&json!({}))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            return Err(self.provider_error(status, &headers, text));
        }
        let session: Value = resp.json().await.map_err(|e| {
            AppError::provider("opencode", format!("invalid session response: {e}"))
        })?;
        let session_id = session
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::provider("opencode", "session response missing `id`"))?
            .to_string();

        // 2. Post the flattened conversation as one message.
        let (system, prompt) = flatten_history(messages);
        let mut msg_body = json!({
            "model": self.model,
            "parts": [{ "type": "text", "text": prompt }],
        });
        if let Some(sys) = system {
            msg_body["system"] = Value::String(sys);
        }
        let resp = self
            .client
            .post(format!("{base}/session/{session_id}/message"))
            .header("content-type", "application/json")
            .json(&msg_body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            return Err(self.provider_error(status, &headers, text));
        }
        let reply: Value = resp.json().await.map_err(|e| {
            AppError::provider("opencode", format!("invalid message response: {e}"))
        })?;

        let mut events = parse_local_parts(reply.get("parts").unwrap_or(&Value::Null));
        events.push(ProviderEvent::Done);
        Ok(Box::pin(futures::stream::iter(events)))
    }

    /// Create a new local session and return its id.
    async fn create_session(&self, base: &str) -> Result<String> {
        let resp = self
            .client
            .post(format!("{base}/session"))
            .header("content-type", "application/json")
            .json(&json!({}))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            return Err(self.provider_error(status, &headers, text));
        }
        let session: Value = resp
            .json()
            .await
            .map_err(|e| AppError::provider("opencode", format!("invalid session response: {e}")))?;
        session
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::provider("opencode", "session response missing `id`"))
    }

    /// Local mode with `persistent_session`: reuse one server session across
    /// turns, sending only new user/tool turns. The server retains prior
    /// context, so the full history is not re-flattened each turn. The session
    /// is recreated when history is cleared (shrinks) or the system prompt
    /// changes; a stale-session HTTP error triggers one transparent retry.
    async fn complete_stream_local_persistent(
        &self,
        messages: &[Message],
    ) -> Result<EventStream<'_>> {
        let base = self.base_url.trim_end_matches('/').to_string();
        let (system, _) = flatten_history(messages);
        let system = system.unwrap_or_default();

        let mut guard = self.session.lock().await;

        // Reset rule: no session, history shrank (cleared), or system changed.
        let needs_new = guard.id.is_none()
            || messages.len() < guard.sent_count
            || guard.sys != system;
        if needs_new {
            let id = self.create_session(&base).await?;
            *guard = PersistState {
                id: Some(id),
                sent_count: 0,
                sys: system.clone(),
            };
        }

        let from = guard.sent_count;
        let new_text = new_turn_text(messages, from);
        let first_message = guard.sent_count == 0;

        let send = |session_id: &str| {
            let mut body = json!({
                "model": self.model,
                "parts": [{ "type": "text", "text": new_text.clone() }],
            });
            if first_message && !system.is_empty() {
                body["system"] = Value::String(system.clone());
            }
            self.client
                .post(format!("{base}/session/{session_id}/message"))
                .header("content-type", "application/json")
                .json(&body)
                .send()
        };

        let session_id = guard.id.clone().expect("session id set above");
        let mut resp = send(&session_id).await?;

        // One transparent retry if the server lost the session.
        if matches!(resp.status().as_u16(), 404 | 400 | 410) {
            let id = self.create_session(&base).await?;
            *guard = PersistState {
                id: Some(id.clone()),
                sent_count: 0,
                sys: system.clone(),
            };
            resp = send(&id).await?;
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            return Err(self.provider_error(status, &headers, text));
        }
        let reply: Value = resp
            .json()
            .await
            .map_err(|e| AppError::provider("opencode", format!("invalid message response: {e}")))?;

        guard.sent_count = messages.len();
        drop(guard);

        let mut events = parse_local_parts(reply.get("parts").unwrap_or(&Value::Null));
        events.push(ProviderEvent::Done);
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

/// Concatenate the new `User` / `ToolResult` turns at and after `from` into one
/// labelled text blob. `Assistant` turns are skipped (the server generates and
/// retains its own assistant reply); `System` is handled via the session.
/// Falls back to the most recent `User` message if nothing new qualifies.
fn new_turn_text(messages: &[Message], from: usize) -> String {
    let mut body = String::new();
    for m in messages.iter().skip(from) {
        match m {
            Message::User { content } => {
                body.push_str("User: ");
                body.push_str(content);
                body.push_str("\n\n");
            }
            Message::ToolResult { content, .. } => {
                body.push_str("Tool result: ");
                body.push_str(content);
                body.push_str("\n\n");
            }
            Message::Assistant { .. } | Message::System { .. } => {}
        }
    }
    let trimmed = body.trim_end();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    // Fallback: resend the latest user message so a reply is still produced.
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::User { content } => Some(content.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// OpenAI chat-completions message mapping (shared cloud-mode shape).
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
            Message::Assistant {
                content,
                tool_calls,
                reasoning_content,
            } => {
                let mut msg = serde_json::Map::new();
                msg.insert("role".into(), json!("assistant"));
                // DeepSeek thinking mode requires the prior assistant turn's
                // chain-of-thought echoed back, else HTTP 400 "The
                // `reasoning_content` in the thinking mode must be passed
                // back to the API."
                if let Some(r) = reasoning_content {
                    msg.insert("reasoning_content".into(), json!(r));
                }
                // OpenAI/DeepSeek accept null content alongside tool_calls;
                // a tool-only turn has no prose.
                msg.insert(
                    "content".into(),
                    if content.is_empty() {
                        Value::Null
                    } else {
                        json!(content)
                    },
                );
                if !tool_calls.is_empty() {
                    let calls: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    // OpenAI requires `arguments` as a JSON string.
                                    "arguments": serde_json::to_string(&tc.arguments)
                                        .unwrap_or_else(|_| "{}".to_string()),
                                }
                            })
                        })
                        .collect();
                    msg.insert("tool_calls".into(), Value::Array(calls));
                }
                out.push(Value::Object(msg));
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

/// Flatten history for the local session API: system messages are joined into
/// the `system` field; the remaining turns are concatenated into one labelled
/// text prompt (the local endpoint takes a single new message's parts).
fn flatten_history(messages: &[Message]) -> (Option<String>, String) {
    let mut system: Option<String> = None;
    let mut body = String::new();
    for m in messages {
        match m {
            Message::System { content } => match system.as_mut() {
                Some(s) => {
                    s.push('\n');
                    s.push_str(content);
                }
                None => system = Some(content.clone()),
            },
            Message::User { content } => {
                body.push_str("User: ");
                body.push_str(content);
                body.push_str("\n\n");
            }
            // Local session-API path (not the shipped cloud config); tool
            // calls are not represented in this flat text form.
            Message::Assistant { content, .. } => {
                body.push_str("Assistant: ");
                body.push_str(content);
                body.push_str("\n\n");
            }
            Message::ToolResult { content, .. } => {
                body.push_str("Tool result: ");
                body.push_str(content);
                body.push_str("\n\n");
            }
        }
    }
    (system, body.trim_end().to_string())
}

/// Intermediate state for cloud-mode (OpenAI-compatible) SSE parsing.
#[derive(Default)]
pub(crate) struct OpenCodeParseState {
    /// Currently assembling tool_call: (id, name, partial JSON args).
    pub current_tool: Option<(String, String, String)>,
}

pub(crate) struct OpenCodeOutcome {
    pub events: Vec<ProviderEvent>,
    pub done: bool,
}

fn flush_tool(state: &mut OpenCodeParseState) -> Option<ProviderEvent> {
    let (id, name, args) = state.current_tool.take()?;
    let v: Value = serde_json::from_str(&args).unwrap_or(Value::Object(Default::default()));
    Some(ProviderEvent::ToolUse { id, name, args: v })
}

/// Pure parser for one cloud-mode (OpenAI-compatible) chat-completions SSE
/// frame. Mirrors the OpenAI streaming shape: text deltas, incrementally
/// assembled `tool_calls`, and a `[DONE]` terminator.
pub(crate) fn handle_opencode_frame(frame: &str, state: &mut OpenCodeParseState) -> OpenCodeOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
        if let Some(ev) = flush_tool(state) {
            events.push(ev);
        }
        events.push(ProviderEvent::Done);
        return OpenCodeOutcome { events, done: true };
    }
    let v: Value = match serde_json::from_str(frame) {
        Ok(v) => v,
        Err(_) => {
            return OpenCodeOutcome {
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
    // DeepSeek thinking mode streams chain-of-thought as
    // `delta.reasoning_content`. Surface it as a Thinking event so
    // process_turn can accumulate and echo it back (the endpoint rejects
    // the next request if the prior assistant turn omits it).
    if let Some(r) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
        if !r.is_empty() {
            events.push(ProviderEvent::Thinking {
                text: r.to_string(),
            });
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
    OpenCodeOutcome {
        events,
        done: false,
    }
}

/// Pure parser for the local session-API synchronous reply's `parts` array.
/// `text` parts become `Text`; `tool` parts become a best-effort `ToolUse`;
/// anything else is skipped.
pub(crate) fn parse_local_parts(parts: &Value) -> Vec<ProviderEvent> {
    let mut events = Vec::new();
    let arr = match parts.as_array() {
        Some(a) => a,
        None => return events,
    };
    for part in arr {
        let ty = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "text" => {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    if !t.is_empty() {
                        events.push(ProviderEvent::Text {
                            delta: t.to_string(),
                        });
                    }
                }
            }
            "tool" => {
                let id = part
                    .get("callID")
                    .or_else(|| part.get("id"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = part
                    .get("tool")
                    .or_else(|| part.get("name"))
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = part
                    .get("state")
                    .and_then(|s| s.get("input"))
                    .or_else(|| part.get("input"))
                    .cloned()
                    .unwrap_or(Value::Object(Default::default()));
                if !name.is_empty() {
                    events.push(ProviderEvent::ToolUse { id, name, args });
                }
            }
            _ => {}
        }
    }
    events
}

#[async_trait]
impl Provider for OpenCodeProvider {
    fn name(&self) -> &'static str {
        "opencode"
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
        if self.api_key.is_some() {
            self.complete_stream_cloud(messages, tools).await
        } else if self.persistent_session {
            self.complete_stream_local_persistent(messages).await
        } else {
            self.complete_stream_local(messages).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_cloud(frames: &[&str]) -> Vec<ProviderEvent> {
        let mut state = OpenCodeParseState::default();
        let mut out = Vec::new();
        for f in frames {
            let outcome = handle_opencode_frame(f, &mut state);
            out.extend(outcome.events);
            if outcome.done {
                break;
            }
        }
        out
    }

    #[test]
    fn cloud_parses_text_chunks_then_done() {
        let evs = collect_cloud(&[
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
    fn cloud_parses_streamed_tool_call() {
        let evs = collect_cloud(&[
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
    fn cloud_invalid_json_is_silently_skipped() {
        let mut state = OpenCodeParseState::default();
        let outcome = handle_opencode_frame("not-json", &mut state);
        assert!(outcome.events.is_empty());
        assert!(!outcome.done);
    }

    #[test]
    fn local_parts_text_and_tool() {
        let parts = json!([
            {"type": "text", "text": "the answer"},
            {"type": "tool", "callID": "t1", "tool": "shell", "state": {"input": {"cmd": "ls"}}},
            {"type": "step-start"}
        ]);
        let evs = parse_local_parts(&parts);
        assert_eq!(evs.len(), 2, "expected text + tool, got {evs:?}");
        match &evs[0] {
            ProviderEvent::Text { delta } => assert_eq!(delta, "the answer"),
            other => panic!("expected Text, got {other:?}"),
        }
        match &evs[1] {
            ProviderEvent::ToolUse { id, name, args } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "shell");
                assert_eq!(args.get("cmd").and_then(|v| v.as_str()), Some("ls"));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
    }

    #[test]
    fn local_parts_non_array_is_empty() {
        assert!(parse_local_parts(&Value::Null).is_empty());
        assert!(parse_local_parts(&json!({"type": "text"})).is_empty());
    }

    #[test]
    fn flatten_history_splits_system_and_dialog() {
        let msgs = vec![
            Message::System {
                content: "be terse".into(),
            },
            Message::User {
                content: "hi".into(),
            },
            Message::Assistant {
                content: "hello".into(),
                tool_calls: vec![],
                reasoning_content: None,
            },
        ];
        let (system, prompt) = flatten_history(&msgs);
        assert_eq!(system.as_deref(), Some("be terse"));
        assert!(prompt.contains("User: hi"));
        assert!(prompt.contains("Assistant: hello"));
    }

    #[test]
    fn to_openai_messages_emits_tool_calls_before_tool_result() {
        // The exact invariant DeepSeek (via opencode) enforces: a
        // `role:"tool"` message must be immediately preceded by an
        // `assistant` message carrying a matching `tool_calls[].id`.
        // Regression guard for the HTTP 400
        // "Messages with role 'tool' must be a response to a preceding
        //  message with 'tool_calls'".
        let msgs = vec![
            Message::System {
                content: "sys".into(),
            },
            Message::User {
                content: "do it".into(),
            },
            Message::Assistant {
                content: String::new(), // tool-only turn: no prose
                tool_calls: vec![crate::ai::ToolCall {
                    id: "call_abc".into(),
                    name: "shell".into(),
                    arguments: json!({"cmd": "ls -la"}),
                }],
                reasoning_content: None,
            },
            Message::ToolResult {
                tool_use_id: "call_abc".into(),
                content: "total 0".into(),
                is_error: false,
            },
        ];
        let out = to_openai_messages(&msgs);
        // [system, user, assistant(tool_calls), tool]
        let asst = &out[2];
        assert_eq!(asst["role"], "assistant");
        assert!(asst["content"].is_null(), "tool-only turn ⇒ null content");
        let calls = asst["tool_calls"].as_array().expect("tool_calls array");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "call_abc");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "shell");
        // OpenAI requires `arguments` as a JSON *string*.
        assert_eq!(
            calls[0]["function"]["arguments"],
            json!(r#"{"cmd":"ls -la"}"#)
        );
        let tool = &out[3];
        assert_eq!(tool["role"], "tool");
        assert_eq!(
            tool["tool_call_id"], calls[0]["id"],
            "tool_call_id must match the preceding assistant tool_calls[].id"
        );
    }

    #[test]
    fn to_openai_messages_assistant_with_text_and_calls() {
        let msgs = vec![
            Message::Assistant {
                content: "I'll run it".into(),
                tool_calls: vec![crate::ai::ToolCall {
                    id: "c1".into(),
                    name: "shell".into(),
                    arguments: json!({}),
                }],
                reasoning_content: None,
            },
            Message::ToolResult {
                tool_use_id: "c1".into(),
                content: "ok".into(),
                is_error: false,
            },
        ];
        let out = to_openai_messages(&msgs);
        assert_eq!(out[0]["content"], "I'll run it");
        assert_eq!(out[0]["tool_calls"][0]["id"], "c1");
        assert_eq!(out[1]["tool_call_id"], "c1");
    }

    #[test]
    fn to_openai_messages_plain_assistant_has_no_tool_calls_key() {
        let out = to_openai_messages(&[Message::Assistant {
            content: "hi".into(),
            tool_calls: vec![],
            reasoning_content: None,
        }]);
        assert_eq!(out[0]["content"], "hi");
        assert!(
            out[0].get("tool_calls").is_none(),
            "no tool_calls key for a plain text reply"
        );
        assert!(
            out[0].get("reasoning_content").is_none(),
            "no reasoning_content key when None"
        );
    }

    #[test]
    fn to_openai_messages_echoes_reasoning_content() {
        // The exact invariant DeepSeek thinking mode enforces: the prior
        // assistant turn must carry its `reasoning_content` back, else
        // HTTP 400 "The `reasoning_content` in the thinking mode must be
        // passed back to the API." Regression guard.
        let msgs = vec![
            Message::Assistant {
                content: String::new(), // reasoning + tool call, no prose
                tool_calls: vec![crate::ai::ToolCall {
                    id: "c9".into(),
                    name: "shell".into(),
                    arguments: json!({"cmd": "ls"}),
                }],
                reasoning_content: Some("let me inspect the workspace".into()),
            },
            Message::ToolResult {
                tool_use_id: "c9".into(),
                content: "ok".into(),
                is_error: false,
            },
        ];
        let out = to_openai_messages(&msgs);
        let asst = &out[0];
        assert_eq!(asst["role"], "assistant");
        assert_eq!(
            asst["reasoning_content"], "let me inspect the workspace",
            "thinking-mode reasoning must be echoed back"
        );
        // Prior-cycle invariants still hold alongside reasoning_content.
        assert!(asst["content"].is_null());
        assert_eq!(asst["tool_calls"][0]["id"], "c9");
        assert_eq!(out[1]["role"], "tool");
        assert_eq!(out[1]["tool_call_id"], "c9");
    }

    #[test]
    fn to_openai_messages_reasoning_only_assistant() {
        let out = to_openai_messages(&[Message::Assistant {
            content: "the answer is 42".into(),
            tool_calls: vec![],
            reasoning_content: Some("42 by deduction".into()),
        }]);
        assert_eq!(out[0]["content"], "the answer is 42");
        assert_eq!(out[0]["reasoning_content"], "42 by deduction");
        assert!(out[0].get("tool_calls").is_none());
    }

    #[test]
    fn cloud_api_parse_defaults_to_openai() {
        assert_eq!(CloudApi::parse(None), CloudApi::OpenAi);
        assert_eq!(CloudApi::parse(Some("openai")), CloudApi::OpenAi);
        assert_eq!(CloudApi::parse(Some("bogus")), CloudApi::OpenAi);
        assert_eq!(CloudApi::parse(Some("  Anthropic ")), CloudApi::Anthropic);
        assert_eq!(CloudApi::parse(Some("messages")), CloudApi::Anthropic);
    }

    #[test]
    fn cloud_url_picks_endpoint_and_trims_slash() {
        assert_eq!(
            cloud_url("https://opencode.ai/zen/go/v1/", CloudApi::OpenAi),
            "https://opencode.ai/zen/go/v1/chat/completions"
        );
        assert_eq!(
            cloud_url("https://opencode.ai/zen/go/v1", CloudApi::Anthropic),
            "https://opencode.ai/zen/go/v1/messages"
        );
    }
}
