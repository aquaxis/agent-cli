//! User-defined custom slash commands loaded from `.md` files in a commands
//! directory (default `.agent-cli/commands`). Each `*.md` file becomes a slash
//! command named after its file stem. Typing `/<name> [args]` in the REPL
//! expands `@file` references and `$ARGUMENTS`/`$N` placeholders in the file
//! content and sends the result to the agent as a user prompt. Mirrors Claude
//! Code's `.claude/commands` mechanism.

use std::path::{Path, PathBuf};

/// A discovered custom command.
#[derive(Debug, Clone)]
pub struct CustomCommand {
    /// File stem, e.g. "hello" for `hello.md`.
    pub name: String,
    /// Absolute or relative path to the `.md` file.
    pub path: PathBuf,
    /// Raw file content (pre-expansion).
    pub content: String,
}

/// Scan `dir` for `*.md` files and return them sorted by name.
///
/// A missing or unreadable directory yields an empty vec (no error) so the
/// REPL keeps working with only built-in commands (FR-03). Unreadable files
/// are skipped (NFR-04).
pub fn discover(dir: &Path) -> Vec<CustomCommand> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        out.push(CustomCommand {
            name: stem,
            path,
            content,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Expand `@<path>` references, `$ARGUMENTS`, and `$1`..`$N` placeholders in
/// `template` and return the expanded prompt text.
///
/// - `$ARGUMENTS` → `args` (the full argument string after the command name).
/// - `$1`, `$2`, ... → the Nth whitespace-separated arg (1-based); an absent
///   argument is replaced with an empty string.
/// - `@<path>` (at start of line or inline) → the referenced file's content;
///   a missing/unreadable file yields `[error: cannot read @<path>]`.
///
/// Expansion order: `$` placeholders first, then `@file` references. This means
/// a `$ARGUMENTS` value that contains `@path` would also be expanded, but file
/// contents that contain `$N` are not re-expanded (single pass).
pub fn expand_template(template: &str, args: &str) -> String {
    let arg_parts: Vec<&str> = args.split_whitespace().collect();
    let mut result = template.replace("$ARGUMENTS", args);
    for (i, part) in arg_parts.iter().enumerate() {
        let placeholder = format!("${}", i + 1);
        result = result.replace(&placeholder, part);
    }
    // Remaining $N placeholders (N > arg count) → empty string.
    let re_n = regex::Regex::new(r"\$\d+").unwrap();
    result = re_n.replace_all(&result, "").to_string();
    // Expand @file references.
    expand_at_refs(&result)
}

fn expand_at_refs(text: &str) -> String {
    let re = regex::Regex::new(r"@([^\s@]+)").unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let p = &caps[1];
        // `config::expand_path` returns `Result<PathBuf>`; handle both the
        // expansion error and the read error with the same placeholder.
        match crate::config::expand_path(p) {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => format!("[error: cannot read @{}]", p),
            },
            Err(_) => format!("[error: cannot read @{}]", p),
        }
    })
    .to_string()
}

/// Resolve a commands directory string from config into a concrete path.
///
/// An empty string falls back to the default (`.agent-cli/commands`). `~` and
/// env-style paths are expanded via `config::expand_path`. Returns the resolved
/// path (which may not exist on disk — discovery handles that gracefully).
pub fn resolve_dir(commands_dir: &str) -> PathBuf {
    let dir = commands_dir.trim();
    let dir = if dir.is_empty() {
        default_commands_dir()
    } else {
        dir.to_string()
    };
    crate::config::expand_path(&dir).unwrap_or_else(|_| PathBuf::from(dir))
}

fn default_commands_dir() -> String {
    ".agent-cli/commands".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &Path, name: &str, content: &str) {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn discover_finds_md_files() {
        let dir = TempDir::new().unwrap();
        write_file(dir.path(), "a.md", "alpha");
        write_file(dir.path(), "b.md", "beta");
        write_file(dir.path(), "c.txt", "ignored");
        let cmds = discover(dir.path());
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(cmds[0].content, "alpha");
    }

    #[test]
    fn discover_missing_dir_returns_empty() {
        let cmds = discover(Path::new("/nonexistent/dir/does/not/exist"));
        assert!(cmds.is_empty());
    }

    #[test]
    fn expand_template_arguments() {
        let out = expand_template("Hello $ARGUMENTS", "world foo");
        assert_eq!(out, "Hello world foo");
    }

    #[test]
    fn expand_template_positional() {
        let out = expand_template("$1 and $2 and $3", "alpha beta");
        assert_eq!(out, "alpha and beta and ");
    }

    #[test]
    fn expand_template_at_file() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "FILE CONTENT").unwrap();
        let template = format!("Before @{} After", target.display());
        let out = expand_template(&template, "");
        assert!(out.contains("FILE CONTENT"), "{}", out);
        assert!(out.starts_with("Before "), "{}", out);
        assert!(out.ends_with(" After"), "{}", out);
    }

    #[test]
    fn expand_template_at_missing_file() {
        let out = expand_template("See @/no/such/path/here.txt", "");
        assert!(out.contains("[error: cannot read @/no/such/path/here.txt]"), "{}", out);
    }

    #[test]
    fn expand_template_combined() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("data.md");
        std::fs::write(&target, "DATA").unwrap();
        let template = format!("@{}\nQuestion: $ARGUMENTS", target.display());
        let out = expand_template(&template, "what is this?");
        assert!(out.contains("DATA"), "{}", out);
        assert!(out.contains("Question: what is this?"), "{}", out);
    }

    #[test]
    fn resolve_dir_default_on_empty() {
        let p = resolve_dir("");
        assert_eq!(p, PathBuf::from(".agent-cli/commands"));
    }

    #[test]
    fn resolve_dir_respects_override() {
        let p = resolve_dir("/tmp/cmds");
        assert_eq!(p, PathBuf::from("/tmp/cmds"));
    }
}