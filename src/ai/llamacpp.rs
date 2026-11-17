use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::ai::openai_chat::to_plain_openai_messages;
use crate::ai::stream::{sse_events, FrameOutcome};
use crate::ai::tool_bridge::to_openai_tools;
use crate::ai::{
    Capabilities, EventStream, Message, Provider, ProviderContext, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};

/// Provider that connects to a llama.cpp server (OpenAI-compatible /v1/chat/completions).
pub struct LlamaCppProvider {
    pub base_url: String,
    pub model: String,
    pub temperature: Option<f32>,
    /// Optional sampling knobs forwarded into the request body. Each maps to a
    /// `llama-cli` flag (see `doc/providers/llamacpp.md`); `None` => omitted
    /// from the body so the llama.cpp server applies its own default.
    pub max_tokens: Option<u32>,
    pub top_k: Option<u32>,
    pub top_p: Option<f32>,
    pub min_p: Option<f32>,
    pub repeat_penalty: Option<f32>,
    pub repeat_last_n: Option<i32>,
    pub seed: Option<u64>,
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
            max_tokens: entry.max_tokens,
            top_k: entry.top_k,
            top_p: entry.top_p,
            min_p: entry.min_p,
            repeat_penalty: entry.repeat_penalty,
            repeat_last_n: entry.repeat_last_n,
            seed: entry.seed,
            client,
            api_key,
            context,
        })
    }

    /// Build the `/v1/chat/completions` request body. Pure (no I/O) so the
    /// sampling-field forwarding can be unit-tested without a live server.
    /// Each optional sampling knob is written only when `Some`; an absent knob
    /// is omitted so the server applies its own default (behavior unchanged
    /// from before these fields existed).
    fn build_body(&self, messages: &[Message], tools: &[ToolSpec]) -> Value {
        let mut body = json!({
            "model": self.model,
            "stream": true,
            "messages": to_plain_openai_messages(messages),
        });
        if !tools.is_empty() {
            body["tools"] = Value::Array(to_openai_tools(tools));
        }
        if let Some(t) = self.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(v) = self.max_tokens {
            body["max_tokens"] = json!(v);
        }
        if let Some(v) = self.top_k {
            body["top_k"] = json!(v);
        }
        if let Some(v) = self.top_p {
            body["top_p"] = json!(v);
        }
        if let Some(v) = self.min_p {
            body["min_p"] = json!(v);
        }
        if let Some(v) = self.repeat_penalty {
            body["repeat_penalty"] = json!(v);
        }
        if let Some(v) = self.repeat_last_n {
            body["repeat_last_n"] = json!(v);
        }
        if let Some(v) = self.seed {
            body["seed"] = json!(v);
        }
        body
    }
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
        let body = self.build_body(messages, tools);

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
        let resp = ensure_success!(
            req.json(&body).send().await?,
            "agent_cli::ai::llamacpp",
            "llama.cpp",
            &self.context
        )?;
        Ok(sse_events(
            resp.bytes_stream(),
            "llama.cpp",
            &self.model,
            handle_llamacpp_frame,
        ))
    }
}

/// Pure function to interpret a single SSE frame from llama.cpp (OpenAI-compatible).
pub(crate) fn handle_llamacpp_frame(frame: &str) -> FrameOutcome {
    let mut events = Vec::new();
    if frame.trim() == "[DONE]" {
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
    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
        if !text.is_empty() {
            events.push(ProviderEvent::Text {
                delta: text.to_string(),
            });
        }
    }
    FrameOutcome {
        events,
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a bare provider (no sampling knobs set), then let `tweak` set
    /// the ones a body test needs.
    fn provider(tweak: impl FnOnce(&mut LlamaCppProvider)) -> LlamaCppProvider {
        let mut p = LlamaCppProvider {
            base_url: "http://127.0.0.1:8080".to_string(),
            model: "default".to_string(),
            temperature: None,
            max_tokens: None,
            top_k: None,
            top_p: None,
            min_p: None,
            repeat_penalty: None,
            repeat_last_n: None,
            seed: None,
            client: reqwest::Client::new(),
            api_key: None,
            context: ProviderContext {
                config_path: PathBuf::new(),
                api_key_env: None,
                api_key_mask: None,
            },
        };
        tweak(&mut p);
        p
    }

    fn one_user() -> Vec<Message> {
        vec![Message::User {
            content: "hi".to_string(),
        }]
    }

    /// With no sampling knobs set, the body carries only the base keys — no
    /// `temperature` and no sampling fields — i.e. byte-compatible with the
    /// behavior before these fields existed.
    #[test]
    fn build_body_omits_unset_sampling_fields() {
        let p = provider(|_| {});
        let body = p.build_body(&one_user(), &[]);
        let obj = body.as_object().unwrap();
        assert_eq!(obj.get("model").and_then(|v| v.as_str()), Some("default"));
        assert_eq!(obj.get("stream").and_then(|v| v.as_bool()), Some(true));
        assert!(obj.contains_key("messages"));
        for k in [
            "temperature",
            "max_tokens",
            "top_k",
            "top_p",
            "min_p",
            "repeat_penalty",
            "repeat_last_n",
            "seed",
            "tools",
        ] {
            assert!(!obj.contains_key(k), "unset field `{k}` must be omitted");
        }
    }

    /// Each set sampling knob is forwarded under its llama.cpp server field
    /// name with the configured value (the instructions.md `-n 1024`,
    /// `--repeat-penalty 1.05`, `--top_k 80` case, plus the rest).
    #[test]
    fn build_body_forwards_set_sampling_fields() {
        let p = provider(|p| {
            p.temperature = Some(0.2);
            p.max_tokens = Some(1024);
            p.top_k = Some(80);
            p.top_p = Some(0.95);
            p.min_p = Some(0.05);
            p.repeat_penalty = Some(1.05);
            p.repeat_last_n = Some(64);
            p.seed = Some(7);
        });
        let body = p.build_body(&one_user(), &[]);
        // Integer fields compare exactly; float fields are compared against the
        // same f32 widening the serializer applies (avoids f32→f64 precision
        // noise, e.g. 0.2_f32 serializing as 0.20000000298…).
        assert_eq!(body["max_tokens"], json!(1024));
        assert_eq!(body["top_k"], json!(80));
        assert_eq!(body["repeat_last_n"], json!(64));
        assert_eq!(body["seed"], json!(7));
        assert_eq!(body["temperature"], json!(0.2_f32));
        assert_eq!(body["top_p"], json!(0.95_f32));
        assert_eq!(body["min_p"], json!(0.05_f32));
        assert_eq!(body["repeat_penalty"], json!(1.05_f32));
    }

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
