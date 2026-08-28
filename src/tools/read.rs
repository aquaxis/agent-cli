use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct ReadTool;

#[derive(Debug, Deserialize)]
struct ReadArgs {
    file_path: String,
    #[serde(default)]
    offset: Option<u64>,
    #[serde(default)]
    limit: Option<u64>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read a UTF-8 text file. Optional offset (1-based start line) and limit (number of lines). Output is line-numbered."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Absolute path to the file to read"},
                "offset": {"type": "integer", "description": "Line number to start reading from (1-based)"},
                "limit": {"type": "integer", "description": "Number of lines to read"}
            },
            "required": ["file_path"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: ReadArgs = serde_json::from_value(args)?;
        let path = expand_path(&parsed.file_path)?;
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
        let text = String::from_utf8_lossy(&bytes);
        // Preserve a trailing newline as an empty final line for numbering.
        let mut lines: Vec<&str> = text.lines().collect();
        if text.ends_with('\n') {
            lines.push("");
        }
        let start = parsed.offset.unwrap_or(1).saturating_sub(1) as usize;
        let start = start.min(lines.len());
        let end = match parsed.limit {
            Some(l) => (start + l as usize).min(lines.len()),
            None => lines.len(),
        };
        let mut out = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            let line_no = start + i + 1;
            out.push_str(&format!("{:>6}\t{}\n", line_no, line));
        }
        // Drop the trailing newline only if we added the synthetic empty line
        // and it was the last appended line (keeps output tidy).
        if out.ends_with("\n\n") {
            out.pop();
        }
        Ok(ToolOutput::ok(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::AgentId;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn ctx() -> ToolCtx {
        ToolCtx {
            self_id: AgentId::new(),
            group: None,
            registry_dir: PathBuf::from("/tmp"),
            config_source: Default::default(),
            event_tx: None,
        }
    }

    fn write_tmp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[tokio::test]
    async fn read_returns_line_numbered_content() {
        let f = write_tmp("alpha\nbeta\ngamma\n");
        let out = ReadTool
            .invoke(
                json!({"file_path": f.path().to_str().unwrap()}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("1\talpha"), "got: {}", out.content);
        assert!(out.content.contains("2\tbeta"));
        assert!(out.content.contains("3\tgamma"));
    }

    #[tokio::test]
    async fn read_with_offset_limit() {
        let f = write_tmp("a\nb\nc\nd\ne\n");
        let out = ReadTool
            .invoke(
                json!({"file_path": f.path().to_str().unwrap(), "offset": 2, "limit": 2}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("2\tb"));
        assert!(out.content.contains("3\tc"));
        assert!(!out.content.contains("1\ta"));
        assert!(!out.content.contains("4\td"));
    }

    #[tokio::test]
    async fn read_binary_file_errors() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&[0xff, 0xfe, 0x00, 0x01]).unwrap();
        let out = ReadTool
            .invoke(json!({"file_path": f.path().to_str().unwrap()}), &ctx())
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("non-UTF-8"));
    }
}