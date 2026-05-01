use std::io;
use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    #[error("config error: {0}")]
    Config(String),

    #[error("config file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("provider error ({provider}): {message}")]
    Provider { provider: String, message: String },

    #[error("tool error ({tool}): {message}")]
    Tool { tool: String, message: String },

    #[error("ipc error: {0}")]
    Ipc(String),

    #[error("registry error: {0}")]
    Registry(String),

    #[error("persona error: {0}")]
    Persona(String),

    #[error("ui error: {0}")]
    Ui(String),

    #[error("agent error: {0}")]
    Agent(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("toml serialize error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("{0}")]
    Other(String),
}

#[allow(dead_code)]
impl AppError {
    pub fn config(msg: impl Into<String>) -> Self {
        AppError::Config(msg.into())
    }

    pub fn provider(provider: impl Into<String>, message: impl Into<String>) -> Self {
        AppError::Provider {
            provider: provider.into(),
            message: message.into(),
        }
    }

    pub fn tool(tool: impl Into<String>, message: impl Into<String>) -> Self {
        AppError::Tool {
            tool: tool.into(),
            message: message.into(),
        }
    }

    pub fn ipc(msg: impl Into<String>) -> Self {
        AppError::Ipc(msg.into())
    }

    pub fn registry(msg: impl Into<String>) -> Self {
        AppError::Registry(msg.into())
    }

    pub fn persona(msg: impl Into<String>) -> Self {
        AppError::Persona(msg.into())
    }

    pub fn agent(msg: impl Into<String>) -> Self {
        AppError::Agent(msg.into())
    }

    pub fn ui(msg: impl Into<String>) -> Self {
        AppError::Ui(msg.into())
    }
}
