use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::expand_path;
use crate::error::Result;
use crate::tools::{Tool, ToolCtx, ToolOutput};

pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default = "default_output_mode")]
    output_mode: String,
    #[serde(default, rename = "-i")]
    case_insensitive: bool,
    #[serde(default = "default_true", rename = "-n")]
    line_numbers: bool,
    #[serde(default, rename = "-A")]
    after: Option<u64>,
    #[serde(default, rename = "-B")]
    before: Option<u64>,
    #[serde(default, rename = "-C")]
    context: Option<u64>,
    #[serde(default)]
    head_limit: Option<u64>,
}

fn default_output_mode() -> String {
    "content".to_string()
}

fn default_true() -> bool {
    true
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file contents for a regex pattern under a path. Supports glob filtering, case-insensitivity, line numbers and context lines. output_mode: content | files_with_matches | count."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Regular expression to search for"},
                "path": {"type": "string", "description": "File or directory to search in (default: current directory)"},
                "glob": {"type": "string", "description": "File-name glob filter"},
                "output_mode": {"type": "string", "enum": ["content", "files_with_matches", "count"], "default": "content"},
                "-i": {"type": "boolean", "description": "Case-insensitive match"},
                "-n": {"type": "boolean", "description": "Show line numbers in content mode", "default": true},
                "-A": {"type": "integer", "description": "Lines of context after a match"},
                "-B": {"type": "integer", "description": "Lines of context before a match"},
                "-C": {"type": "integer", "description": "Lines of context around a match"},
                "head_limit": {"type": "integer", "description": "Limit number of result entries"}
            },
            "required": ["pattern"]
        })
    }

    async fn invoke(&self, args: Value, _ctx: &ToolCtx) -> Result<ToolOutput> {
        let parsed: GrepArgs = serde_json::from_value(args)?;
        let re = {
            let mut b = regex::RegexBuilder::new(&parsed.pattern);
            b.case_insensitive(parsed.case_insensitive);
            match b.build() {
                Ok(r) => r,
                Err(e) => return Ok(ToolOutput::err(format!("invalid regex: {e}"))),
            }
        };
        let glob_matcher = parsed.glob.as_deref().map(|g| {
            globset::GlobBuilder::new(g)
                .literal_separator(true)
                .build()
                .map(|g| g.compile_matcher())
        });
        let glob_matcher = match glob_matcher {
            Some(Ok(m)) => Some(m),
            Some(Err(e)) => return Ok(ToolOutput::err(format!("invalid glob: {e}"))),
            None => None,
        };

        let base = match &parsed.path {
            Some(p) => expand_path(p)?,
            None => std::env::current_dir()?,
        };
        let after = parsed.context.unwrap_or(0).max(parsed.after.unwrap_or(0)) as usize;
        let before = parsed.context.unwrap_or(0).max(parsed.before.unwrap_or(0)) as usize;

        let mut files: Vec<String> = Vec::new();
        let mut lines_out: Vec<String> = Vec::new();
        let mut count: usize = 0;

        let entries: Vec<std::path::PathBuf> = if base.is_file() {
            vec![base.clone()]
        } else {
            walkdir::WalkDir::new(&base)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.path().to_path_buf())
                .collect()
        };

        for path in entries {
            if let Some(m) = &glob_matcher {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if !m.is_match(&name) {
                    continue;
                }
            }
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue, // skip binary/unreadable
            };
            if std::str::from_utf8(content.as_bytes()).is_err() {
                continue;
            }
            let all_lines: Vec<&str> = content.lines().collect();
            let display = path
                .strip_prefix(&base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let mut file_matched = false;
            for (i, line) in all_lines.iter().enumerate() {
                if re.is_match(line) {
                    count += 1;
                    file_matched = true;
                    if matches!(parsed.output_mode.as_str(), "content") {
                        let lo = i.saturating_sub(before);
                        let hi = (i + 1 + after).min(all_lines.len());
                        for (j, ctx_line) in all_lines[lo..hi].iter().enumerate() {
                            let ln = lo + j + 1;
                            if parsed.line_numbers {
                                lines_out.push(format!("{}:{}:{}", display, ln, ctx_line));
                            } else {
                                lines_out.push(format!("{}:{}", display, ctx_line));
                            }
                        }
                    }
                }
            }
            if file_matched {
                files.push(display);
            }
        }

        let body = match parsed.output_mode.as_str() {
            "count" => count.to_string(),
            "files_with_matches" => {
                let mut v = files;
                v.sort();
                v.dedup();
                v.join("\n")
            }
            "content" => {
                let mut v = lines_out;
                if let Some(limit) = parsed.head_limit {
                    v.truncate(limit as usize);
                }
                v.join("\n")
            }
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
            registry_dir: PathBuf::from("/tmp"),
            event_tx: None,
        }
    }

    async fn setup() -> TempDir {
        let dir = TempDir::new().unwrap();
        tokio::fs::write(dir.path().join("a.txt"), "alpha\nbeta\nGAMMA\n").await.unwrap();
        tokio::fs::write(dir.path().join("b.log"), "gamma here\n").await.unwrap();
        dir
    }

    #[tokio::test]
    async fn grep_content_mode() {
        let dir = setup().await;
        let out = GrepTool
            .invoke(
                json!({"pattern": "beta", "path": dir.path().to_str().unwrap()}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        assert!(out.content.contains("a.txt:2:beta"), "{}", out.content);
    }

    #[tokio::test]
    async fn grep_files_with_matches_mode() {
        let dir = setup().await;
        let out = GrepTool
            .invoke(
                json!({"pattern": "gamma", "path": dir.path().to_str().unwrap(), "output_mode": "files_with_matches"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.ok);
        // case-sensitive: only b.log has lowercase "gamma"
        assert!(out.content.contains("b.log"));
        assert!(!out.content.contains("a.txt"));
    }

    #[tokio::test]
    async fn grep_count_mode() {
        let dir = setup().await;
        let out = GrepTool
            .invoke(
                json!({"pattern": "gamma", "path": dir.path().to_str().unwrap(), "output_mode": "count", "-i": true}),
                &ctx(),
            )
            .await
            .unwrap();
        assert_eq!(out.content, "2");
    }

    #[tokio::test]
    async fn grep_case_insensitive() {
        let dir = setup().await;
        let out = GrepTool
            .invoke(
                json!({"pattern": "gamma", "path": dir.path().to_str().unwrap(), "-i": true}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("GAMMA"));
    }

    #[tokio::test]
    async fn grep_glob_filter() {
        let dir = setup().await;
        let out = GrepTool
            .invoke(
                json!({"pattern": "gamma", "path": dir.path().to_str().unwrap(), "-i": true, "glob": "*.txt", "output_mode": "files_with_matches"}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.content.contains("a.txt"));
        assert!(!out.content.contains("b.log"));
    }
}