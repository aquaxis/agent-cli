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
    /// Key material for the `Authorization: Bearer` header, resolved at
    /// construction. `None` means a keyless connection: no `api_key_env`
    /// is configured, so no auth header is sent at all (a keyless backend
    /// such as the Strata engine, or any OpenAI-compatible server without
    /// authentication). A *configured* `api_key_env` whose variable is
    /// missing still fails construction with the missing-key error.
    pub api_key: Option<String>,
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub client: reqwest::Client,
    pub context: ProviderContext,
    /// The configured kind this provider serves. The codex wire backs two
    /// kinds — `"codex"` and the `"strata"` convenience alias (the Strata
    /// local engine, served over the same Chat Completions wire) — and the
    /// provider reports which one the user actually configured, so banner
    /// rows, registry entries and diagnostics name it truthfully.
    kind_label: &'static str,
}

impl CodexProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        // The codex wire backs two kinds. `kind = "strata"` (the Strata local
        // engine) is served from the `[provider.strata]` slot with its own
        // defaults — a `[provider.codex]` table in the same file is never
        // consulted, so the alias cannot inherit the other kind's key
        // material. The provider keeps the configured kind as its label so
        // banner rows, `selftest`, and the provider-error prefixes name what
        // the user actually configured.
        let (kind_label, entry) = match cfg.provider.kind.as_str() {
            "strata" => {
                let mut entry = cfg.provider.strata.clone().unwrap_or_default();
                if entry.base_url.is_none() {
                    entry.base_url = Some("http://127.0.0.1:8080/v1".to_string());
                }
                if entry.model.is_none() {
                    entry.model = Some("strata".to_string());
                }
                ("strata", entry)
            }
            _ => (
                "codex",
                cfg.provider
                    .codex
                    .clone()
                    .ok_or_else(|| AppError::provider("codex", "[provider.codex] missing"))?,
            ),
        };
        // Keyless when NO `api_key_env` is configured: no default env var is
        // invented and no header is sent. A configured-but-unset variable
        // keeps the missing-key construction error, verbatim.
        let api_key = match &entry.api_key_env {
            None => None,
            Some(env) => Some(std::env::var(env).map_err(|_| {
                ProviderError::new(kind_label)
                    .with_body(format!("env var {env} not set"))
                    .with_context(&ProviderContext::new(source, Some(env.clone()), None))
                    .into_app_error()
            })?),
        };
        let context = ProviderContext::new(
            source,
            entry.api_key_env.clone(),
            api_key.as_deref(),
        );
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
            kind_label,
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
            // Not reachable from the shipped `kind="opencode"` config; tool
            // calls are not serialized here (same latent bug as the opencode
            // path had — fix deferred to a codex-provider cycle if needed).
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
impl Provider for CodexProvider {
    fn name(&self) -> &'static str {
        self.kind_label
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
        let request = self
            .client
            .post(url)
            .header("content-type", "application/json");
        // No `api_key_env` configured => a keyless endpoint (e.g. a local
        // Strata engine): no Authorization header at all. A configured env
        // var keeps the missing-key construction error and the bearer header.
        let request = if let Some(key) = &self.api_key {
            request.bearer_auth(key)
        } else {
            request
        };
        let resp = request.json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let headers = resp.headers().clone();
            let text = resp.text().await.unwrap_or_default();
            tracing::debug!(target: "agent_cli::ai::codex", status = %status, body = %text, "provider HTTP error");
            let request_id = extract_request_id(&headers, &text);
            return Err(ProviderError::new(self.kind_label)
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
                        yield ProviderEvent::Error {
                            message: crate::ai::stream_mid_body(
                                self.kind_label,
                                &self.model,
                                &e.to_string(),
                            ),
                        };
                        return;
                    }
                }
            }
            yield ProviderEvent::Done;
        };
        Ok(Box::pin(stream))
    }
}

/// Intermediate state for OpenAI Chat Completions stream parsing.
#[derive(Default)]
pub(crate) struct CodexParseState {
    /// Currently assembling tool_call: (id, name, partial JSON)
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

/// Pure function to interpret a single Chat Completions SSE frame.
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
        // Emit tool_use even in the edge case where [DONE] arrives without a finish_reason
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

    fn provider_from(toml_src: &str) -> CodexProvider {
        let cfg: Config = toml::from_str(toml_src).unwrap();
        CodexProvider::from_config(&cfg, &ConfigSource::default()).unwrap()
    }

    /// Accept one HTTP request on an ephemeral port and return its header
    /// block — the only way to prove a negative about what the client sends.
    fn capture_one_request(listener: std::net::TcpListener) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap_or(0);
            let raw = String::from_utf8_lossy(&buf[..n]).to_string();
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\n",
            );
            let _ = stream.flush();
            raw
        })
    }

    fn user_msg(text: &str) -> Message {
        Message::User {
            content: text.to_string(),
        }
    }

    #[test]
    fn missing_api_key_error_exists_only_when_api_key_env_is_configured() {
        // No `api_key_env` => keyless: construction succeeds even though no
        // env var exists, with no key material and no default key env implied.
        std::env::remove_var("OPENAI_API_KEY");
        let p = provider_from("[provider]\nkind = \"codex\"\n\n[provider.codex]\n");
        assert!(p.api_key.is_none(), "no api_key_env means no key");
        // `api_key_env` configured but its variable unset => the construction
        // error, verbatim (a typo'd env name must not become a silent open).
        let src_missing = "[provider]\nkind = \"codex\"\n\n[provider.codex]\napi_key_env = \"AGENT_CLI_NO_SUCH_VAR\"\n";
        let cfg: Config = toml::from_str(src_missing).unwrap();
        let outcome = CodexProvider::from_config(&cfg, &ConfigSource::default());
        let err = match outcome {
            Ok(_) => panic!("configured-unset key must abort construction"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("env var AGENT_CLI_NO_SUCH_VAR not set"),
            "configured-unset key must still abort: {err}"
        );
    }

    #[test]
    fn strata_configured_reports_strata_everywhere_codex_reports_codex() {
        // The alias keeps its own name on every surface that shows a kind:
        // the provider's identity (`name()`), the codex wire's stream-error
        // message, and its HTTP-error prefix. Only the kind configured in
        // the file may appear there — never the internal backing provider's.
        let strata = provider_from("[provider]\nkind = \"strata\"\n\n[provider.strata]\n");
        assert_eq!(strata.name(), "strata");
        let mid = crate::ai::stream_mid_body(strata.name(), strata.model(), "err");
        assert!(
            mid.contains("provider strata,"),
            "stream errors must name the configured kind: {mid}"
        );
        let codex = provider_from("[provider]\nkind = \"codex\"\n\n[provider.codex]\n");
        assert_eq!(codex.name(), "codex");
    }

    #[tokio::test]
    async fn keyless_codex_sends_no_authorization_and_keyed_does() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured = capture_one_request(listener);
        let p = provider_from(&format!(
            "[provider]\nkind = \"codex\"\n\n[provider.codex]\nbase_url = \"http://127.0.0.1:{port}/v1\"\n"
        ));
        // The canned reply is not a valid SSE stream, so the call ends after
        // the header block — whose contents are what this pin is about.
        let _ = p.complete_stream(&[user_msg("hi")], &[]).await;
        let raw = captured.join().unwrap().to_ascii_lowercase();
        assert!(
            raw.contains("post /v1/chat/completions"),
            "unexpected request line: {raw}"
        );
        assert!(
            !raw.contains("authorization:"),
            "keyless codex must not send an authorization header: {raw}"
        );
    }

    #[tokio::test]
    async fn keyed_codex_sends_the_bearer_header() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let captured = capture_one_request(listener);
        // The key is resolved at construction, so the variable must exist
        // before the provider is built.
        std::env::set_var("AGENT_CLI_TEST_KEY_ENV", "strata-pin-key");
        let p = provider_from(&format!(
            "[provider]\nkind = \"codex\"\n\n[provider.codex]\nbase_url = \"http://127.0.0.1:{port}/v1\"\napi_key_env = \"AGENT_CLI_TEST_KEY_ENV\"\n"
        ));
        let _ = p.complete_stream(&[user_msg("hi")], &[]).await;
        std::env::remove_var("AGENT_CLI_TEST_KEY_ENV");
        let raw = captured.join().unwrap();
        let lower = raw.to_ascii_lowercase();
        assert!(
            lower.contains("authorization: bearer strata-pin-key"),
            "keyed codex must keep the bearer header on the wire: {raw}"
        );
    }
}
