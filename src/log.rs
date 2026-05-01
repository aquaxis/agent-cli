use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::error::Result;
use crate::id::AgentId;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum LogEvent<'a> {
    User {
        text: &'a str,
    },
    Assistant {
        text: &'a str,
    },
    Thinking {
        text: &'a str,
    },
    ToolCall {
        name: &'a str,
        args: &'a serde_json::Value,
    },
    ToolResult {
        name: &'a str,
        ok: bool,
        output: &'a str,
    },
    PeerPrompt {
        from: &'a str,
        text: &'a str,
    },
    System {
        message: &'a str,
    },
}

pub struct ConversationLog {
    file: Arc<Mutex<tokio::fs::File>>,
    #[allow(dead_code)]
    pub path: PathBuf,
}

impl ConversationLog {
    pub async fn open(log_dir: &std::path::Path, id: &AgentId) -> Result<Self> {
        let dir = log_dir.join(id.as_str());
        tokio::fs::create_dir_all(&dir).await?;
        let stamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let path = dir.join(format!("{stamp}.jsonl"));
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            path,
        })
    }

    pub async fn write(&self, event: LogEvent<'_>) -> Result<()> {
        #[derive(Serialize)]
        struct Wrapped<'a> {
            ts: String,
            #[serde(flatten)]
            event: LogEvent<'a>,
        }
        let line = serde_json::to_string(&Wrapped {
            ts: Utc::now().to_rfc3339(),
            event,
        })?;
        let mut f = self.file.lock().await;
        f.write_all(line.as_bytes()).await?;
        f.write_all(b"\n").await?;
        Ok(())
    }
}
