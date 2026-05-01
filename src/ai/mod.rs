use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{AppError, Result};

pub mod claude;
pub mod codex;
pub mod llamacpp;
pub mod ollama;
pub mod stream;
pub mod tool_bridge;

/// バックエンドが提供する機能の有無。
///
/// REPL や `selftest` はこの情報を使って未対応機能の警告や代替表示を行う。
#[derive(Debug, Clone, Copy, Default)]
pub struct Capabilities {
    /// ストリーミング応答に対応するか。
    pub streaming: bool,
    /// `tool_use`／function calling 相当の機能をネイティブに発火できるか。
    pub tool_use: bool,
    /// thinking ブロック（内省ステップ）を別系統で受信できるか。
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
    /// 同期的に外部から差し込まれたツール実行結果
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

/// AI バックエンド抽象。各バックエンドは `complete_stream` で
/// `ProviderEvent` 列を返し、上位の Agent 会話ループはバックエンド固有の表現を
/// 知ることなく対話・ツール呼び出しを進められる。
#[async_trait]
pub trait Provider: Send + Sync {
    /// バックエンド識別子。`"claude"` / `"codex"` / `"ollama"` / `"llama.cpp"`。
    fn name(&self) -> &'static str;
    /// 当該バックエンドが提供する機能の有無。
    fn capabilities(&self) -> Capabilities;
    /// 現在使用中のモデル名。
    fn model(&self) -> &str;
    /// 与えられた会話履歴とツール定義から、ストリーミングで `ProviderEvent` を返す。
    async fn complete_stream(
        &self,
        messages: &[Message],
        tools: &[ToolSpec],
    ) -> Result<EventStream<'_>>;
}

pub fn build(cfg: &Config) -> Result<Box<dyn Provider>> {
    let kind = cfg.provider.kind.as_str();
    match kind {
        "claude" => Ok(Box::new(claude::ClaudeProvider::from_config(cfg)?)),
        "codex" => Ok(Box::new(codex::CodexProvider::from_config(cfg)?)),
        "ollama" => Ok(Box::new(ollama::OllamaProvider::from_config(cfg)?)),
        "llama.cpp" => Ok(Box::new(llamacpp::LlamaCppProvider::from_config(cfg)?)),
        other => Err(AppError::provider(other, "unknown provider kind")),
    }
}

pub const SUPPORTED: &[&str] = &["claude", "codex", "ollama", "llama.cpp"];

#[cfg(test)]
pub mod testing {
    //! テスト用ヘルパー：スクリプト化された `ProviderEvent` 列を返す `MockProvider`。
    use std::sync::Mutex;

    use super::*;

    pub struct MockProvider {
        pub model: String,
        scripts: Mutex<Vec<Vec<ProviderEvent>>>,
    }

    impl MockProvider {
        /// `scripts[i]` は i 回目の `complete_stream` 呼び出しで放出するイベント列。
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
