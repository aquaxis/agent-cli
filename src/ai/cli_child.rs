//! Helpers shared by the backends that drive a local CLI as a child process
//! (`claude-cli`, `codex-cli`): executable resolution and config-enum parsing.

use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

/// Parse an optional `[provider.<provider>] <key>` string against `table`
/// (case-insensitive, trimmed). `None` yields `default`; an unknown value is a
/// provider error listing the accepted spellings.
pub(crate) fn parse_enum<T: Copy>(
    provider: &'static str,
    key: &str,
    value: Option<&str>,
    table: &[(&str, T)],
    default: T,
) -> Result<T> {
    let Some(raw) = value else { return Ok(default) };
    let needle = raw.trim().to_ascii_lowercase();
    if let Some((_, v)) = table.iter().find(|(name, _)| *name == needle) {
        return Ok(*v);
    }
    let accepted = table
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    Err(AppError::provider(
        provider,
        format!("[provider.{provider}] {key}: unknown value \"{raw}\" (accepted: {accepted})"),
    ))
}

/// Resolve the configured executable: used as-is when it looks like a path,
/// otherwise searched on `PATH`. `product` names the tool in the not-found
/// message ("Claude Code") and `install` is its remediation lead-in
/// ("Install Claude Code").
pub(crate) fn resolve_bin(
    raw: &str,
    provider: &'static str,
    product: &str,
    install: &str,
) -> Result<PathBuf> {
    let not_found = || {
        AppError::provider(
            provider,
            format!(
                "{product} executable not found: \"{raw}\".\n  \
                 {install}, or set [provider.{provider}] bin to its full path."
            ),
        )
    };
    let expanded = shellexpand::tilde(raw).to_string();
    let candidate = Path::new(&expanded);
    if expanded.contains(std::path::MAIN_SEPARATOR) {
        return is_executable(candidate)
            .then(|| candidate.to_path_buf())
            .ok_or_else(not_found);
    }
    let paths = std::env::var_os("PATH").ok_or_else(not_found)?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(&expanded))
        .find(|p| is_executable(p))
        .ok_or_else(not_found)
}

fn is_executable(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}
