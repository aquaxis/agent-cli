use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct WriteTool;

#[derive(Debug, Deserialize)]
struct WriteArgs {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write UTF-8 text to a file, overwriting any existing content. Parent directories are created."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Absolute path to the file to write"},
                "content": {"type": "string", "description": "Content to write to the file"}
            },
            "required": ["file_path", "content"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: WriteArgs = serde_json::from_value(args)?;
        let path = expand_path(&parsed.file_path)?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        tokio::fs::write(&path, parsed.content.as_bytes()).await?;
        Ok(ToolOutput::ok(format!("wrote {}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::AgentId;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn ctx() -> ToolCtx {
        ToolCtx {
            self_id: AgentId::new(),
            group: None,
            registry_dir: PathBuf::from("/tmp"),
            config_source: Default::default(),
            event_tx: None,
        }
    }

    #[tokio::test]
    async fn write_creates_file() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("out.txt");
        let out = WriteTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "content": "hello"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn write_overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("out.txt");
        tokio::fs::write(&p, "old").await.unwrap();
        let out = WriteTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "content": "new"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "new");
    }

    #[tokio::test]
    async fn write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("a/b/c.txt");
        let out = WriteTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "content": "x"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "x");
    }
}