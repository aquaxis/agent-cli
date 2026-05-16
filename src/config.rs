use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

const DEFAULT_CONFIG: &str = r#"# agent-cli configuration

[provider]
# Backend to use: "claude" | "codex" | "ollama" | "opencode" | "llama.cpp"
kind = "claude"

[provider.claude]
model       = "claude-opus-4-7"
api_key_env = "ANTHROPIC_API_KEY"
base_url    = "https://api.anthropic.com"
thinking    = true
# Opt-in: Anthropic prompt caching (system + tools + conversation tail).
# prompt_cache = true

[provider.codex]
model       = "gpt-4.1"
api_key_env = "OPENAI_API_KEY"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider.opencode]
# Local: point base_url at a running `opencode serve` (no api_key_env needed).
# Cloud (OpenCode Zen): set base_url to the Zen endpoint and api_key_env to the
# environment variable holding your key. A resolved key selects cloud mode.
model    = "claude-sonnet-4-5"
base_url = "http://127.0.0.1:4096"
# api_key_env = "OPENCODE_API_KEY"
# base_url    = "https://opencode.ai/zen/v1"
# Opt-in (local mode only): reuse one server session across turns.
# persistent_session = true

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"
registry_dir       = ""
agents_dir         = "~/.config/agent-cli/agents"
persona_file       = ""

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 60
max_output_kb = 256

[ui]
show_thinking = "collapsed"

[history]
# Opt-in hybrid window management. When disabled (default), the full
# conversation is replayed verbatim each turn (unchanged behavior).
enabled            = false
max_context_tokens = 24000
keep_recent_turns  = 6
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: ProviderRoot,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub history: HistoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRoot {
    pub kind: String,
    #[serde(default)]
    pub claude: Option<ProviderEntry>,
    #[serde(default)]
    pub codex: Option<ProviderEntry>,
    #[serde(default)]
    pub ollama: Option<ProviderEntry>,
    #[serde(default)]
    pub opencode: Option<ProviderEntry>,
    #[serde(default, rename = "llama.cpp")]
    pub llamacpp: Option<ProviderEntry>,
    #[serde(flatten)]
    pub extras: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEntry {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub thinking: Option<bool>,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Total HTTP request timeout in seconds, including streaming. Optional —
    /// providers fall back to a generous default (≥ 900s) so cloud reasoning
    /// models (e.g. `glm-5.1:cloud`) that emit minutes of `thinking` tokens
    /// before content do not get aborted mid-stream.
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
    /// Claude only: enable Anthropic prompt caching (`cache_control`
    /// breakpoints on system / tools / conversation tail). Opt-in;
    /// `None`/absent => disabled (behavior unchanged).
    #[serde(default)]
    pub prompt_cache: Option<bool>,
    /// opencode local mode only: reuse one OpenCode `session_id` across turns
    /// and send only new turns instead of re-flattening full history. Opt-in;
    /// `None`/absent => disabled (ephemeral session per turn, unchanged).
    #[serde(default)]
    pub persistent_session: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub auto_approve_tools: bool,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    #[serde(default)]
    pub registry_dir: String,
    #[serde(default = "default_agents_dir")]
    pub agents_dir: String,
    #[serde(default)]
    pub persona_file: String,
    /// Maximum tool-use iterations per peer prompt. Each iteration is one
    /// (LLM call → optional tool calls + their results) round. The default
    /// of 8 is too low for design-then-debug workflows where the agent
    /// generates artifacts, runs validators, and iterates on lint feedback.
    /// Bump to 16+ for orchestrators that own multiple tools per turn.
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: u32,
}

fn default_max_tool_iterations() -> u32 {
    24
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            auto_approve_tools: false,
            log_dir: default_log_dir(),
            registry_dir: String::new(),
            agents_dir: default_agents_dir(),
            persona_file: String::new(),
            max_tool_iterations: default_max_tool_iterations(),
        }
    }
}

fn default_log_dir() -> String {
    "~/.local/share/agent-cli/logs".to_string()
}

fn default_agents_dir() -> String {
    "~/.config/agent-cli/agents".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_tools_enabled")]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub shell: ShellToolConfig,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: default_tools_enabled(),
            shell: ShellToolConfig::default(),
        }
    }
}

fn default_tools_enabled() -> Vec<String> {
    vec![
        "shell".to_string(),
        "fs_read".to_string(),
        "fs_write".to_string(),
        "send_to".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellToolConfig {
    #[serde(default = "default_shell_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_shell_max_output")]
    pub max_output_kb: u64,
}

impl Default for ShellToolConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_shell_timeout(),
            max_output_kb: default_shell_max_output(),
        }
    }
}

fn default_shell_timeout() -> u64 {
    60
}

fn default_shell_max_output() -> u64 {
    256
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_show_thinking")]
    pub show_thinking: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_thinking: default_show_thinking(),
        }
    }
}

fn default_show_thinking() -> String {
    "collapsed".to_string()
}

/// `[history]` — hybrid history-window management. Opt-in (`enabled = false`
/// by default); when disabled the conversation is replayed verbatim as before.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Master switch. When false, no summarization or trimming occurs.
    #[serde(default)]
    pub enabled: bool,
    /// Approximate context budget (estimated tokens ≈ chars/4). When the
    /// estimated history exceeds this, compaction runs.
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
    /// Number of most-recent turns always kept verbatim (never summarized or
    /// dropped). Leading system/persona messages are always kept too.
    #[serde(default = "default_keep_recent_turns")]
    pub keep_recent_turns: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_context_tokens: default_max_context_tokens(),
            keep_recent_turns: default_keep_recent_turns(),
        }
    }
}

fn default_max_context_tokens() -> usize {
    24_000
}

fn default_keep_recent_turns() -> usize {
    6
}

/// Display mode for `[ui] show_thinking` (FR-03-1-2 / design doc 4.3C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowThinkingMode {
    /// Do not show thinking at all.
    Hidden,
    /// Show up to the first 80 characters of each delta, truncating the rest with `...`.
    Collapsed,
    /// Show the full text as received.
    Expanded,
}

impl UiConfig {
    /// Normalize the `show_thinking` string into a `ShowThinkingMode`. Unknown values
    /// fall back to the default `Collapsed` (does not prevent startup on parse errors).
    pub fn show_thinking_mode(&self) -> ShowThinkingMode {
        match self.show_thinking.as_str() {
            "hidden" => ShowThinkingMode::Hidden,
            "expanded" => ShowThinkingMode::Expanded,
            "collapsed" => ShowThinkingMode::Collapsed,
            _ => ShowThinkingMode::Collapsed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub path: PathBuf,
    pub from_explicit: bool,
}

pub fn default_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::config("could not resolve user config directory"))?;
    Ok(base.join("agent-cli").join("config.toml"))
}

pub fn resolve_path(explicit: Option<&Path>) -> Result<ConfigSource> {
    if let Some(p) = explicit {
        return Ok(ConfigSource {
            path: expand_path(p.to_string_lossy().as_ref())?,
            from_explicit: true,
        });
    }
    Ok(ConfigSource {
        path: default_path()?,
        from_explicit: false,
    })
}

pub fn load(source: &ConfigSource) -> Result<Config> {
    if !source.path.exists() {
        if source.from_explicit {
            return Err(AppError::ConfigNotFound(source.path.clone()));
        }
        // Default path -> auto-generate
        if let Some(parent) = source.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&source.path, DEFAULT_CONFIG)?;
        tracing::info!(path = %source.path.display(), "default config generated");
    }
    let raw = std::fs::read_to_string(&source.path)?;
    let cfg: Config = toml::from_str(&raw)?;
    Ok(cfg)
}

pub fn expand_path(p: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(p)
        .map_err(|e| AppError::config(format!("path expansion failed: {e}")))?;
    Ok(PathBuf::from(expanded.into_owned()))
}

/// Format an API key value for masked display (FR-09-3).
///
/// Keys of 8 or more characters are shown as "first 4 chars + `...` + last 4 chars".
/// Shorter keys return `***` to avoid leaking the value length.
/// Empty strings return an empty string (callers should check `Option` first to
/// distinguish "not set").
pub fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 8 {
        return "***".to_string();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}...{tail}")
}

impl Config {
    pub fn provider_entry(&self, kind: &str) -> Option<&ProviderEntry> {
        match kind {
            "claude" => self.provider.claude.as_ref(),
            "codex" => self.provider.codex.as_ref(),
            "ollama" => self.provider.ollama.as_ref(),
            "opencode" => self.provider.opencode.as_ref(),
            "llama.cpp" => self.provider.llamacpp.as_ref(),
            _ => None,
        }
    }

    pub fn apply_overrides(&mut self, provider: Option<&str>, model: Option<&str>) {
        if let Some(p) = provider {
            self.provider.kind = p.to_string();
        }
        if let Some(m) = model {
            if let Some(entry) = self.provider_entry_mut(&self.provider.kind.clone()) {
                entry.model = Some(m.to_string());
            }
        }
    }

    pub fn apply_persona_overrides(&mut self, model: Option<&str>, temperature: Option<f32>) {
        let kind = self.provider.kind.clone();
        if let Some(entry) = self.provider_entry_mut(&kind) {
            if let Some(m) = model {
                entry.model = Some(m.to_string());
            }
            if let Some(t) = temperature {
                entry.temperature = Some(t);
            }
        }
    }

    fn provider_entry_mut(&mut self, kind: &str) -> Option<&mut ProviderEntry> {
        match kind {
            "claude" => Some(self.provider.claude.get_or_insert_with(Default::default)),
            "codex" => Some(self.provider.codex.get_or_insert_with(Default::default)),
            "ollama" => Some(self.provider.ollama.get_or_insert_with(Default::default)),
            "opencode" => Some(self.provider.opencode.get_or_insert_with(Default::default)),
            "llama.cpp" => Some(self.provider.llamacpp.get_or_insert_with(Default::default)),
            _ => None,
        }
    }

    pub fn registry_dir(&self) -> Result<PathBuf> {
        if !self.runtime.registry_dir.is_empty() {
            return expand_path(&self.runtime.registry_dir);
        }
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !dir.is_empty() {
                return Ok(PathBuf::from(dir).join("agent-cli"));
            }
        }
        Ok(PathBuf::from("/tmp/agent-cli"))
    }

    pub fn log_dir(&self) -> Result<PathBuf> {
        expand_path(&self.runtime.log_dir)
    }

    pub fn agents_dir(&self) -> Result<PathBuf> {
        expand_path(&self.runtime.agents_dir)
    }
}

#[cfg(test)]
pub(crate) fn tests_default_config() -> &'static str {
    DEFAULT_CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_config() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).expect("default config must parse");
        assert_eq!(cfg.provider.kind, "claude");
        assert!(cfg.provider.claude.is_some());
        assert!(cfg.provider.ollama.is_some());
        assert!(cfg.provider.llamacpp.is_some());
        assert_eq!(cfg.tools.enabled.len(), 4);
        assert_eq!(cfg.tools.shell.timeout_secs, 60);
    }

    /// FR-04-3 boundary (upper limit): TOML with `max_tool_iterations` set to `u32::MAX`
    /// parses successfully and the value is preserved. We don't actually loop 4 billion
    /// times (parse success alone guarantees that true unlimited is disallowed but
    /// practically unlimited is achievable).
    #[test]
    fn max_tool_iterations_accepts_u32_max() {
        let toml_src = r#"
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
[runtime]
max_tool_iterations = 4294967295
"#;
        let cfg: Config = toml::from_str(toml_src).expect("u32::MAX must parse");
        assert_eq!(cfg.runtime.max_tool_iterations, u32::MAX);
    }

    /// FR-03-1-2 / design doc 4.3C: `[ui] show_thinking` string parsing.
    /// Verify the 3 known values (`hidden`/`collapsed`/`expanded`) and unknown-value fallback.
    #[test]
    fn show_thinking_mode_parses_known_values() {
        for (raw, expected) in [
            ("hidden", ShowThinkingMode::Hidden),
            ("collapsed", ShowThinkingMode::Collapsed),
            ("expanded", ShowThinkingMode::Expanded),
        ] {
            let ui = UiConfig {
                show_thinking: raw.into(),
            };
            assert_eq!(ui.show_thinking_mode(), expected, "raw={raw}");
        }
    }

    #[test]
    fn show_thinking_mode_unknown_value_falls_back_to_collapsed() {
        let ui = UiConfig {
            show_thinking: "verbose".into(),
        };
        assert_eq!(ui.show_thinking_mode(), ShowThinkingMode::Collapsed);
        // Unspecified (default) also equals Collapsed.
        assert_eq!(
            UiConfig::default().show_thinking_mode(),
            ShowThinkingMode::Collapsed
        );
    }

    /// Default value of `max_tool_iterations` is 24 (raised from 8 on 2026-05-03).
    /// Omitting the `[runtime]` section also yields the same default.
    #[test]
    fn max_tool_iterations_default_is_24() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(cfg.runtime.max_tool_iterations, 24);

        let minimal = r#"
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let cfg2: Config = toml::from_str(minimal).unwrap();
        assert_eq!(cfg2.runtime.max_tool_iterations, 24);
    }

    #[test]
    fn override_provider_and_model() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        cfg.apply_overrides(Some("ollama"), Some("glm-5.1:cloud"));
        assert_eq!(cfg.provider.kind, "ollama");
        assert_eq!(
            cfg.provider.ollama.as_ref().and_then(|p| p.model.clone()),
            Some("glm-5.1:cloud".into())
        );
    }

    #[test]
    fn mask_api_key_handles_edge_cases() {
        // Empty string: returns empty as-is (caller distinguishes "not set" via Option)
        assert_eq!(mask_api_key(""), "");
        // Short key: always returns *** to avoid leaking length
        assert_eq!(mask_api_key("abc"), "***");
        assert_eq!(mask_api_key("1234567"), "***");
        // 8+ characters: first 4 + ... + last 4
        assert_eq!(mask_api_key("12345678"), "1234...5678");
        assert_eq!(mask_api_key("sk-ant-api03-XYZ-abcdef"), "sk-a...cdef");
        // Also works with typical Anthropic key length (~108 chars)
        let long: String = "sk-ant-"
            .chars()
            .chain(std::iter::repeat_n('x', 100))
            .chain("nQAA".chars())
            .collect();
        let masked = mask_api_key(&long);
        assert!(masked.starts_with("sk-a..."));
        assert!(masked.ends_with("nQAA"));
    }

    #[test]
    fn persona_overrides_apply_to_active_provider() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        cfg.apply_overrides(Some("ollama"), None);
        cfg.apply_persona_overrides(Some("custom-model"), Some(0.4));
        let entry = cfg.provider.ollama.as_ref().expect("ollama entry");
        assert_eq!(entry.model.as_deref(), Some("custom-model"));
        assert_eq!(entry.temperature, Some(0.4));
    }

    /// Documentation consistency check (T-602-10):
    /// Verify that the 3 complete sample configs from `doc/config.md` parse as `Config`.
    #[test]
    fn doc_config_md_full_samples_parse() {
        let minimal = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let recommended = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
thinking    = true

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 120
max_output_kb = 512

[ui]
show_thinking = "collapsed"
"#;
        let full = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"
registry_dir       = "/tmp/agent-cli"
agents_dir         = "~/.config/agent-cli/agents"
persona_file       = ""

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 60
max_output_kb = 256

[ui]
show_thinking = "expanded"
"#;
        for (label, body) in [
            ("minimal", minimal),
            ("recommended", recommended),
            ("full", full),
        ] {
            let cfg: Config = toml::from_str(body)
                .unwrap_or_else(|e| panic!("doc config sample '{label}' failed to parse: {e}"));
            assert!(
                !cfg.provider.kind.is_empty(),
                "{label}: provider.kind missing"
            );
        }
    }

    /// Verify `tools.enabled` names match the implemented tools (prevent typos).
    #[test]
    fn enabled_tool_names_match_implementation() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let known = ["shell", "fs_read", "fs_write", "send_to"];
        for name in &cfg.tools.enabled {
            assert!(
                known.contains(&name.as_str()),
                "unknown tool name in DEFAULT_CONFIG: {name}"
            );
        }
    }
}
