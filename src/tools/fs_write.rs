use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct FsWriteTool;

#[derive(Debug, Deserialize)]
struct FsWriteArgs {
    path: String,
    content: String,
    #[serde(default)]
    overwrite: bool,
}

#[async_trait]
impl Tool for FsWriteTool {
    fn name(&self) -> &'static str {
        "fs_write"
    }

    fn description(&self) -> &'static str {
        "Write UTF-8 text to a file. By default refuses to overwrite existing files; pass overwrite=true to force."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"},
                "overwrite": {"type": "boolean"},
            },
            "required": ["path", "content"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: FsWriteArgs = serde_json::from_value(args)?;
        let path = expand_path(&parsed.path)?;
        if path.exists() && !parsed.overwrite {
            return Ok(ToolOutput::err(format!(
                "{} already exists; pass overwrite=true to replace",
                path.display()
            )));
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        tokio::fs::write(&path, parsed.content.as_bytes()).await?;
        Ok(ToolOutput::ok(format!("wrote {}", path.display())))
    }
}
