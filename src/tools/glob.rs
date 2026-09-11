use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct GlobTool;

#[derive(Debug, Deserialize)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_output_mode")]
    output_mode: String,
}

fn default_output_mode() -> String {
    "content".to_string()
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern (supports *, **, ?, [..]) under a directory. Returns a sorted list of matching paths."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern"},
                "path": {"type": "string", "description": "Directory to search in (default: current directory)"},
                "output_mode": {"type": "string", "enum": ["content", "files_with_matches", "count"], "description": "Result format", "default": "content"}
            },
            "required": ["pattern"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: GlobArgs = serde_json::from_value(args)?;
        let base = match &parsed.path {
            Some(p) => expand_path(p)?,
            None => std::env::current_dir()?,
        };
        let glob = match globset::GlobBuilder::new(&parsed.pattern)
            .literal_separator(true)
            .build()
        {
            Ok(g) => g.compile_matcher(),
            Err(e) => return Ok(ToolOutput::err(format!("invalid pattern: {e}"))),
        };
        let mut matches: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(&base).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&base)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();
            if glob.is_match(&rel) || glob.is_match(entry.path()) {
                matches.push(entry.path().to_string_lossy().to_string());
            }
        }
        matches.sort();
        matches.dedup();
        let body = match parsed.output_mode.as_str() {
            "count" => matches.len().to_string(),
            "files_with_matches" | "content" => matches.join("\n"),
            other => return Ok(ToolOutput::err(format!("invalid output_mode: {other}"))),
        };
        Ok(ToolOutput::ok(body))
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
            ancestors: Vec::new(),
            spawn_limits: crate::swarm::SpawnLimits::default(),
        }
    }

    async fn setup() -> TempDir {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("a.txt"), "x").await.unwrap();
        tokio::fs::write(dir.path().join("b.rs"), "x").await.unwrap();
        tokio::fs::create_dir(dir.path().join("sub")).await.unwrap();
        tokio::fs::write(dir.path().join("sub").join("c.txt"), "x")
            .await
            .unwrap();
        dir
    }

    #[tokio::test]
    async fn glob_matches_pattern() {
        let dir = setup().await;
        let out = GlobTool
            .invoke(
                json!({"pattern": "**/*.txt", "path": dir.path().to_str().unwrap()}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        let content = out.content;
        assert!(content.contains("a.txt"));
        assert!(content.contains("c.txt"));
        assert!(!content.contains("b.rs"));
    }

    #[tokio::test]
    async fn glob_returns_sorted() {
        let dir = setup().await;
        let out = GlobTool
            .invoke(
                json!({"pattern": "*", "path": dir.path().to_str().unwrap()}),
                &ctx(),
            )
            .await
            .unwrap();
        // `*` matches only top-level files (literal separator), returned as
        // full sorted paths.
        let lines: Vec<&str> = out.content.lines().collect();
        assert_eq!(lines.len(), 2, "content: {}", out.content);
        assert!(lines[0].ends_with("a.txt"));
        assert!(lines[1].ends_with("b.rs"));
    }

    #[tokio::test]
    async fn glob_count_mode() {
        let dir = setup().await;
        let out = GlobTool
            .invoke(
                json!({"pattern": "**/*.txt", "path": dir.path().to_str().unwrap(), "output_mode": "count"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert_eq!(out.content, "2");
    }
}