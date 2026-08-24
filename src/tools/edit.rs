use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct EditTool;

#[derive(Debug, Deserialize)]
struct EditArgs {
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Perform an exact string replacement in a file. By default old_string must be unique; set replace_all=true to replace every occurrence."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Absolute path to the file to modify"},
                "old_string": {"type": "string", "description": "Exact text to replace (must be unique unless replace_all)"},
                "new_string": {"type": "string", "description": "Text to replace it with"},
                "replace_all": {"type": "boolean", "description": "Replace all occurrences", "default": false}
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: EditArgs = serde_json::from_value(args)?;
        if parsed.old_string == parsed.new_string {
            return Ok(ToolOutput::err("old_string and new_string must differ"));
        }
        let path = expand_path(&parsed.file_path)?;
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolOutput::err(format!("read error: {e}"))),
        };
        let count = content.matches(&parsed.old_string).count();
        if count == 0 {
            return Ok(ToolOutput::err(format!(
                "old_string not found in {}",
                path.display()
            )));
        }
        if !parsed.replace_all && count > 1 {
            return Ok(ToolOutput::err(format!(
                "old_string is not unique ({count} occurrences) in {}; pass replace_all=true to replace all",
                path.display()
            )));
        }
        let new_content = if parsed.replace_all {
            content.replace(&parsed.old_string, &parsed.new_string)
        } else {
            content.replacen(&parsed.old_string, &parsed.new_string, 1)
        };
        tokio::fs::write(&path, new_content.as_bytes()).await?;
        Ok(ToolOutput::ok(format!(
            "replaced {} occurrence(s) in {}",
            if parsed.replace_all { count } else { 1 },
            path.display()
        )))
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
            registry_dir: PathBuf::from("/tmp"),
            config_source: Default::default(),
            event_tx: None,
        }
    }

    async fn setup(content: &str) -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("f.txt");
        tokio::fs::write(&p, content).await.unwrap();
        (dir, p)
    }

    #[tokio::test]
    async fn edit_unique_replace() {
        let (dir, p) = setup("foo bar baz").await;
        let out = EditTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "old_string": "bar", "new_string": "QUX"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok, "{}", out.content);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "foo QUX baz");
        drop(dir);
    }

    #[tokio::test]
    async fn edit_replace_all() {
        let (dir, p) = setup("x x x").await;
        let out = EditTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "old_string": "x", "new_string": "y", "replace_all": true}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(tokio::fs::read_to_string(&p).await.unwrap(), "y y y");
        drop(dir);
    }

    #[tokio::test]
    async fn edit_non_unique_errors() {
        let (dir, p) = setup("a a a").await;
        let out = EditTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "old_string": "a", "new_string": "b"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("not unique"));
        drop(dir);
    }

    #[tokio::test]
    async fn edit_missing_old_string_errors() {
        let (dir, p) = setup("hello").await;
        let out = EditTool
            .invoke(
                json!({"file_path": p.to_str().unwrap(), "old_string": "nope", "new_string": "x"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(!out.ok);
        assert!(out.content.contains("not found"));
        drop(dir);
    }
}