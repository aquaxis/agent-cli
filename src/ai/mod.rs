use std::fmt;
use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::config::{Config, ConfigSource};
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

pub fn build(cfg: &Config, source: &ConfigSource) -> Result<Box<dyn Provider>> {
    let kind = cfg.provider.kind.as_str();
    match kind {
        "claude" => Ok(Box::new(claude::ClaudeProvider::from_config(cfg, source)?)),
        "codex" => Ok(Box::new(codex::CodexProvider::from_config(cfg, source)?)),
        "ollama" => Ok(Box::new(ollama::OllamaProvider::from_config(cfg, source)?)),
        "llama.cpp" => Ok(Box::new(llamacpp::LlamaCppProvider::from_config(
            cfg, source,
        )?)),
        other => Err(AppError::provider(other, "unknown provider kind")),
    }
}

pub const SUPPORTED: &[&str] = &["claude", "codex", "ollama", "llama.cpp"];

/// プロバイダ HTTP エラー時の診断情報（FR-09-3／設計書 5.1）。
///
/// 4xx／5xx 応答を受領した際にユーザーが原因を切り分けられるよう、
/// 当該プロバイダが認識している周辺情報（解決済み設定ファイルパス・
/// `api_key_env` 名・APIキーのマスク表示・`request_id`・特定パターンに対する
/// ヒント）を一括で表示用に整形する。
#[derive(Debug, Clone)]
pub struct ProviderError {
    pub provider: String,
    pub status: Option<u16>,
    pub status_text: Option<String>,
    pub body: String,
    pub request_id: Option<String>,
    pub config_path: Option<PathBuf>,
    pub api_key_env: Option<String>,
    pub api_key_mask: Option<String>,
    pub hint: Option<String>,
}

impl ProviderError {
    pub fn new(provider: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            status: None,
            status_text: None,
            body: String::new(),
            request_id: None,
            config_path: None,
            api_key_env: None,
            api_key_mask: None,
            hint: None,
        }
    }

    pub fn with_http(mut self, status: u16, status_text: impl Into<String>) -> Self {
        self.status = Some(status);
        self.status_text = Some(status_text.into());
        self
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn with_context(mut self, ctx: &ProviderContext) -> Self {
        self.config_path = Some(ctx.config_path.clone());
        self.api_key_env = ctx.api_key_env.clone();
        self.api_key_mask = ctx.api_key_mask.clone();
        self
    }

    /// 応答パターンを解析してヒント文を埋め込む。
    pub fn detect_hint(mut self) -> Self {
        self.hint = derive_hint(self.status, &self.body);
        self
    }

    /// `AppError::provider(...)` への文字列ペイロードとして、多行サマリ形式で返す。
    pub fn into_app_error(self) -> AppError {
        let provider = self.provider.clone();
        AppError::provider(provider, self.to_string())
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 1 行目：HTTP ステータスサマリ
        match (self.status, &self.status_text) {
            (Some(code), Some(text)) => writeln!(f, "HTTP {code} {text}")?,
            (Some(code), None) => writeln!(f, "HTTP {code}")?,
            (None, Some(text)) => writeln!(f, "{text}")?,
            (None, None) => {}
        }
        if let Some(rid) = &self.request_id {
            writeln!(f, "  request_id : {rid}")?;
        }
        if let Some(p) = &self.config_path {
            writeln!(f, "  config     : {}", p.display())?;
        }
        match (&self.api_key_env, &self.api_key_mask) {
            (Some(env), Some(mask)) if !mask.is_empty() => {
                writeln!(f, "  api_key_env: {env} ({mask})")?
            }
            (Some(env), _) => writeln!(f, "  api_key_env: {env} (not set)")?,
            _ => {}
        }
        if !self.body.is_empty() {
            // body は長尺になりうるので 1 行に潰して 1KB で打ち切る
            let one_line = self.body.replace('\n', " ");
            let trimmed: String = one_line.chars().take(1024).collect();
            writeln!(f, "  detail     : {trimmed}")?;
        }
        if let Some(hint) = &self.hint {
            writeln!(f, "  hint       : {hint}")?;
        }
        Ok(())
    }
}

/// プロバイダごとの診断コンテキスト（解決済み設定ファイルパス・`api_key_env`・キーマスク）。
#[derive(Debug, Clone)]
pub struct ProviderContext {
    pub config_path: PathBuf,
    pub api_key_env: Option<String>,
    pub api_key_mask: Option<String>,
}

impl ProviderContext {
    pub fn new(
        source: &ConfigSource,
        api_key_env: Option<String>,
        api_key_value: Option<&str>,
    ) -> Self {
        let api_key_mask = api_key_value.map(crate::config::mask_api_key);
        Self {
            config_path: source.path.clone(),
            api_key_env,
            api_key_mask,
        }
    }
}

/// HTTP ステータスと応答本文から「特定パターン → 対処ヒント」の対応を返す。
pub fn derive_hint(status: Option<u16>, body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    if lower.contains("credit balance is too low") {
        return Some(
            "Anthropic アカウントのクレジット残高が不足しています。\
             https://console.anthropic.com/settings/billing で確認・購入するか、\
             別アカウントの API キーを `api_key_env` の指す環境変数に設定してください。"
                .to_string(),
        );
    }
    if status == Some(401)
        || lower.contains("invalid_api_key")
        || lower.contains("authentication_error")
        || lower.contains("invalid x-api-key")
    {
        return Some(
            "API キーが無効または失効しています。\
             `api_key_env` の指す環境変数の値を確認するか、\
             プロバイダのコンソールから再発行してください。"
                .to_string(),
        );
    }
    if status == Some(429) || lower.contains("rate_limit") || lower.contains("rate limit") {
        return Some(
            "レート制限に達しました。数分待ってから再試行するか、\
             より低頻度の呼び出しに切り替えてください。"
                .to_string(),
        );
    }
    if matches!(status, Some(s) if (500..600).contains(&s)) {
        return Some(
            "プロバイダ側の一時的な障害が疑われます。\
             しばらく待ってから再試行してください。"
                .to_string(),
        );
    }
    None
}

/// 応答ヘッダーまたは本文 JSON から `request_id` を抽出する。
///
/// - ヘッダー：`request-id`／`x-request-id` を優先（大文字小文字は不問）。
/// - 本文 JSON：トップレベル `request_id`、または `error.request_id`、`id` を順に試す。
pub fn extract_request_id(headers: &reqwest::header::HeaderMap, body: &str) -> Option<String> {
    for name in ["request-id", "x-request-id"] {
        if let Some(v) = headers.get(name) {
            if let Ok(s) = v.to_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        for path in [
            &["request_id"][..],
            &["error", "request_id"][..],
            &["id"][..],
        ] {
            let mut cur = &value;
            let mut ok = true;
            for key in path {
                match cur.get(*key) {
                    Some(v) => cur = v,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                if let Some(s) = cur.as_str() {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn hint_for_credit_balance_too_low() {
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."}}"#;
        let hint = derive_hint(Some(400), body).expect("hint");
        assert!(hint.contains("クレジット残高"));
        assert!(hint.contains("billing"));
    }

    #[test]
    fn hint_for_authentication_error() {
        let body = r#"{"error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
        let hint = derive_hint(Some(401), body).expect("hint");
        assert!(hint.contains("API キー"));
    }

    #[test]
    fn hint_for_rate_limit() {
        let hint = derive_hint(Some(429), "rate_limit_exceeded").expect("hint");
        assert!(hint.contains("レート制限"));
    }

    #[test]
    fn hint_for_server_error() {
        let hint = derive_hint(Some(503), "service unavailable").expect("hint");
        assert!(hint.contains("一時的"));
    }

    #[test]
    fn hint_none_for_unknown_400() {
        // 「クレジット不足」「認証エラー」のいずれにも当てはまらない 400 はヒントなし。
        assert!(derive_hint(Some(400), "something else").is_none());
    }

    #[test]
    fn extract_request_id_from_header() {
        let mut h = HeaderMap::new();
        h.insert("request-id", HeaderValue::from_static("req_abc123"));
        let rid = extract_request_id(&h, "{}").expect("rid");
        assert_eq!(rid, "req_abc123");
    }

    #[test]
    fn extract_request_id_from_x_header_when_no_request_id() {
        let mut h = HeaderMap::new();
        h.insert("x-request-id", HeaderValue::from_static("x_xyz_999"));
        let rid = extract_request_id(&h, "{}").expect("rid");
        assert_eq!(rid, "x_xyz_999");
    }

    #[test]
    fn extract_request_id_from_body_when_no_header() {
        let h = HeaderMap::new();
        let body = r#"{"type":"error","error":{},"request_id":"req_011Caej"}"#;
        let rid = extract_request_id(&h, body).expect("rid");
        assert_eq!(rid, "req_011Caej");
    }

    #[test]
    fn extract_request_id_returns_none_when_missing() {
        let h = HeaderMap::new();
        assert!(extract_request_id(&h, "not-json").is_none());
        assert!(extract_request_id(&h, "{}").is_none());
    }

    #[test]
    fn provider_error_display_contains_all_fields() {
        let pe = ProviderError::new("claude")
            .with_http(400, "Bad Request")
            .with_body(r#"{"error":{"message":"Your credit balance is too low to access the Anthropic API."}}"#)
            .with_request_id(Some("req_abc".to_string()))
            .with_context(&ProviderContext {
                config_path: PathBuf::from("/home/u/.config/agent-cli/config.toml"),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_key_mask: Some("sk-a...nQAA".to_string()),
            })
            .detect_hint();
        let s = pe.to_string();
        assert!(s.contains("HTTP 400"));
        assert!(s.contains("req_abc"));
        assert!(s.contains("/home/u/.config/agent-cli/config.toml"));
        assert!(s.contains("ANTHROPIC_API_KEY"));
        assert!(s.contains("sk-a...nQAA"));
        assert!(s.contains("クレジット残高"));
        // 機密漏洩防止：マスクされていないキー本体が含まれないこと
        assert!(!s.contains("sk-ant-fullkey"));
    }

    #[test]
    fn provider_error_display_marks_unset_key() {
        let pe = ProviderError::new("claude")
            .with_http(401, "Unauthorized")
            .with_context(&ProviderContext {
                config_path: PathBuf::from("/c/agent-cli/config.toml"),
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                api_key_mask: None,
            });
        let s = pe.to_string();
        assert!(s.contains("ANTHROPIC_API_KEY (not set)"));
    }
}

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
