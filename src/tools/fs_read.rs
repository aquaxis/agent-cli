use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct FsReadTool;

#[derive(Debug, Deserialize)]
struct FsReadArgs {
    path: String,
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    limit: Option<u64>,
}

#[async_trait]
impl Tool for FsReadTool {
    fn name(&self) -> &'static str {
        "fs_read"
    }

    fn description(&self) -> &'static str {
        "Read a UTF-8 text file from the filesystem. Optional offset (bytes) and limit (bytes)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer"}
            },
            "required": ["path"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: FsReadArgs = serde_json::from_value(args)?;
        let path = expand_path(&parsed.path)?;
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(e) => return Ok(ToolOutput::err(format!("read error: {e}"))),
        };
        if std::str::from_utf8(&bytes).is_err() {
            return Ok(ToolOutput::err(format!(
                "binary or non-UTF-8 file: {}",
                path.display()
            )));
        }
        let start = parsed.offset.unwrap_or(0) as usize;
        let end = match parsed.limit {
            Some(l) => (start + l as usize).min(bytes.len()),
            None => bytes.len(),
        };
        let start = start.min(bytes.len());
        let slice = &bytes[start..end];
        Ok(ToolOutput::ok(String::from_utf8_lossy(slice).to_string()))
    }
}
