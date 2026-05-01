use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

const DEFAULT_CONFIG: &str = r#"# agent-cli configuration

[provider]
# 使用するバックエンド: "claude" | "codex" | "ollama" | "llama.cpp"
kind = "claude"

[provider.claude]
model       = "claude-opus-4-7"
api_key_env = "ANTHROPIC_API_KEY"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
model       = "gpt-4.1"
api_key_env = "OPENAI_API_KEY"
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
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            auto_approve_tools: false,
            log_dir: default_log_dir(),
            registry_dir: String::new(),
            agents_dir: default_agents_dir(),
            persona_file: String::new(),
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
        // Default path → 自動生成
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

impl Config {
    pub fn provider_entry(&self, kind: &str) -> Option<&ProviderEntry> {
        match kind {
            "claude" => self.provider.claude.as_ref(),
            "codex" => self.provider.codex.as_ref(),
            "ollama" => self.provider.ollama.as_ref(),
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
    fn persona_overrides_apply_to_active_provider() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        cfg.apply_overrides(Some("ollama"), None);
        cfg.apply_persona_overrides(Some("custom-model"), Some(0.4));
        let entry = cfg.provider.ollama.as_ref().expect("ollama entry");
        assert_eq!(entry.model.as_deref(), Some("custom-model"));
        assert_eq!(entry.temperature, Some(0.4));
    }

    /// ドキュメント整合性チェック（T-602-10）：
    /// `doc/config.md` に記載した完全サンプル 3 種が `Config` として正しくパースできること。
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

    /// `tools.enabled` の名前が実装ツールと一致していること（typo 防止）。
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
