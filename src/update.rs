//! Self-update: `agent-cli update`.
//!
//! Discovers the latest released version on GitHub and — since agent-cli ships
//! no prebuilt binaries and is installed by a source build (`install.sh` →
//! `cargo install`) — upgrades by rebuilding from the released tag with
//! `cargo install --git … --tag …` into the running binary's install prefix,
//! then verifies the replacement. Linux-only; requires the Rust toolchain.
//!
//! The decision logic (semver compare, repo-slug parse, tag extraction,
//! install-prefix derivation) is factored into pure functions so it is
//! unit-testable without touching the network or `cargo`.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

pub struct UpdateOpts {
    pub check: bool,
    pub force: bool,
    pub yes: bool,
    /// `--ref`: a tag (`vX.Y.Z`) or a branch name. `None` → the latest release.
    pub git_ref: Option<String>,
}

/// Entry point for the `update` subcommand.
pub async fn run(opts: UpdateOpts) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let repo_url = env!("CARGO_PKG_REPOSITORY");
    let (owner, repo) = repo_slug(repo_url)
        .ok_or_else(|| AppError::Other(format!("cannot parse repository from {repo_url:?}")))?;

    let client = reqwest::Client::builder()
        .user_agent(format!("agent-cli/{current}"))
        .build()?;

    // Latest-tag lookup. Required for `--check` and the default (no `--ref`)
    // path; only informational when an explicit `--ref` is given, so a
    // `--ref main` update still works on a repo with no published releases.
    let latest = fetch_latest_tag(&client, &owner, &repo).await;
    match &latest {
        Ok(tag) => println!("agent-cli {current}  (latest: {tag})"),
        Err(_) => println!("agent-cli {current}  (latest: unknown)"),
    }

    if opts.check {
        let tag = latest?;
        println!(
            "update available: {}",
            if is_newer(&tag, current) { "yes" } else { "no" }
        );
        return Ok(());
    }

    // Choose the target ref: an explicit --ref, else the latest release tag.
    let target = match opts.git_ref.clone() {
        Some(r) => r,
        None => {
            let tag = latest.map_err(|e| {
                AppError::Other(format!(
                    "{e}\nhint: pass --ref <branch|tag> to update from a specific ref (e.g. --ref main)"
                ))
            })?;
            if !is_newer(&tag, current) && !opts.force {
                println!("already up to date ({current})");
                return Ok(());
            }
            tag
        }
    };

    let exe =
        std::env::current_exe().map_err(|e| AppError::Other(format!("current_exe: {e}")))?;
    let prefix = install_prefix(&exe).ok_or_else(|| {
        AppError::Other(format!(
            "cannot derive install prefix from {}",
            exe.display()
        ))
    })?;

    // Confirm before replacing the binary (unless told to proceed).
    if !opts.yes && !opts.force {
        use std::io::IsTerminal;
        if std::io::stdin().is_terminal() {
            use std::io::Write;
            print!(
                "update agent-cli {current} -> {target} into {}? [y/N] ",
                prefix.display()
            );
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|e| AppError::Other(format!("failed to read confirmation: {e}")))?;
            let ans = line.trim().to_ascii_lowercase();
            if ans != "y" && ans != "yes" {
                println!("aborted.");
                return Ok(());
            }
        } else {
            return Err(AppError::Other(
                "refusing to update without a TTY; pass --yes to proceed".into(),
            ));
        }
    }

    install_via_cargo(&owner, &repo, &target, &prefix).await?;

    let bin = prefix.join("bin").join("agent-cli");
    verify(&bin).await
}

/// Fetch the latest released tag: `releases/latest`, falling back to `tags`
/// (newest by semver) when the repo has no formal releases (404).
async fn fetch_latest_tag(client: &reqwest::Client, owner: &str, repo: &str) -> Result<String> {
    let rel_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let resp = client
        .get(&rel_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?;
    if resp.status().is_success() {
        let body = resp.text().await?;
        return tag_from_release_json(&body)
            .ok_or_else(|| AppError::Other("no tag_name in releases/latest response".into()));
    }
    if resp.status().as_u16() == 404 {
        let tags_url = format!("https://api.github.com/repos/{owner}/{repo}/tags");
        let resp2 = client
            .get(&tags_url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await?;
        if !resp2.status().is_success() {
            return Err(AppError::Other(format!(
                "GitHub tags API returned {}",
                resp2.status()
            )));
        }
        let body = resp2.text().await?;
        return newest_tag_from_tags_json(&body).ok_or_else(|| {
            AppError::Other(format!(
                "no published releases or vX.Y.Z tags for {owner}/{repo}"
            ))
        });
    }
    Err(AppError::Other(format!(
        "GitHub releases API returned {}",
        resp.status()
    )))
}

/// Rebuild + install the target ref via `cargo install`. Retries once without
/// `--locked` (mirrors install.sh). A missing `cargo` yields rustup guidance.
async fn install_via_cargo(owner: &str, repo: &str, target: &str, prefix: &Path) -> Result<()> {
    let url = format!("https://github.com/{owner}/{repo}");
    let ref_flag = if is_branch_ref(target) {
        "--branch"
    } else {
        "--tag"
    };
    let prefix_str = prefix.to_string_lossy().to_string();
    println!("installing agent-cli {target} via cargo into {prefix_str} ...");

    match spawn_cargo(&url, ref_flag, target, &prefix_str, true).await {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => {
            // Retry without --locked, as install.sh does.
            let status = spawn_cargo(&url, ref_flag, target, &prefix_str, false)
                .await
                .map_err(map_cargo_err)?;
            if status.success() {
                Ok(())
            } else {
                Err(AppError::Other("cargo install failed".into()))
            }
        }
        Err(e) => Err(map_cargo_err(e)),
    }
}

async fn spawn_cargo(
    url: &str,
    ref_flag: &str,
    target: &str,
    prefix_str: &str,
    locked: bool,
) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("install")
        .arg("--git")
        .arg(url)
        .arg(ref_flag)
        .arg(target)
        .arg("agent-cli")
        .arg("--root")
        .arg(prefix_str)
        .arg("--force");
    if locked {
        cmd.arg("--locked");
    }
    cmd.status().await
}

fn map_cargo_err(e: std::io::Error) -> AppError {
    if e.kind() == std::io::ErrorKind::NotFound {
        AppError::Other(
            "cargo not found on PATH. Install the Rust toolchain first:\n  \
             curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                .into(),
        )
    } else {
        AppError::Other(format!("failed to run cargo: {e}"))
    }
}

/// Verify by running the freshly installed binary's `--version`.
async fn verify(bin: &Path) -> Result<()> {
    let out = tokio::process::Command::new(bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| AppError::Other(format!("failed to run {}: {e}", bin.display())))?;
    if !out.status.success() {
        return Err(AppError::Other(
            "updated binary did not report a version".into(),
        ));
    }
    println!("updated: {}", String::from_utf8_lossy(&out.stdout).trim());
    Ok(())
}

// ---------------------------------------------------------------------------
// Pure helpers (no I/O) — unit-tested below.
// ---------------------------------------------------------------------------

/// Parse `owner`/`repo` from a GitHub repository URL. Non-github → `None`.
fn repo_slug(url: &str) -> Option<(String, String)> {
    let u = url.trim().trim_end_matches('/');
    let u = u.strip_suffix(".git").unwrap_or(u);
    let rest = u.split("github.com/").nth(1)?;
    let mut parts = rest.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

/// Parse `MAJOR.MINOR.PATCH` (leading `v` stripped, a `-pre`/`+build` suffix on
/// the patch ignored). Requires three components.
fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim();
    let s = s
        .strip_prefix('v')
        .or_else(|| s.strip_prefix('V'))
        .unwrap_or(s);
    let mut it = s.split('.');
    let major = it.next()?.parse::<u64>().ok()?;
    let minor = it.next()?.parse::<u64>().ok()?;
    let patch = leading_u64(it.next()?)?;
    Some((major, minor, patch))
}

fn leading_u64(s: &str) -> Option<u64> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn cmp_semver(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_semver(a)?.cmp(&parse_semver(b)?))
}

/// True iff `latest` is a strictly greater semver than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    matches!(cmp_semver(latest, current), Some(Ordering::Greater))
}

fn tag_from_release_json(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("tag_name")?.as_str().map(|s| s.to_string())
}

/// Highest `vX.Y.Z` tag from the `/tags` array — by semver, not list order.
fn newest_tag_from_tags_json(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let arr = v.as_array()?;
    let mut best: Option<((u64, u64, u64), String)> = None;
    for item in arr {
        let name = match item.get("name").and_then(|n| n.as_str()) {
            Some(n) => n,
            None => continue,
        };
        if let Some(ver) = parse_semver(name) {
            if best.as_ref().map_or(true, |(bv, _)| ver > *bv) {
                best = Some((ver, name.to_string()));
            }
        }
    }
    best.map(|(_, name)| name)
}

/// `<prefix>/bin/agent-cli` → `<prefix>`.
fn install_prefix(exe: &Path) -> Option<PathBuf> {
    let bin_dir = exe.parent()?; // <prefix>/bin
    let prefix = bin_dir.parent()?; // <prefix>
    Some(prefix.to_path_buf())
}

/// A ref that is not a `vX.Y.Z` semver is treated as a branch name.
fn is_branch_ref(r: &str) -> bool {
    parse_semver(r).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_slug_parses_github_https() {
        assert_eq!(
            repo_slug("https://github.com/aquaxis/agent-cli"),
            Some(("aquaxis".into(), "agent-cli".into()))
        );
        assert_eq!(
            repo_slug("https://github.com/aquaxis/agent-cli.git/"),
            Some(("aquaxis".into(), "agent-cli".into()))
        );
        assert_eq!(repo_slug("https://example.com/x/y"), None);
    }

    #[test]
    fn parse_semver_variants() {
        assert_eq!(parse_semver("v0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_semver("0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_semver("v1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.6"), None);
        assert_eq!(parse_semver("x"), None);
    }

    #[test]
    fn is_newer_compares_semver() {
        assert!(is_newer("v0.7.0", "0.6.0"));
        assert!(is_newer("0.6.1", "0.6.0"));
        assert!(is_newer("1.0.0", "0.6.0"));
        assert!(!is_newer("0.6.0", "0.6.0"));
        assert!(!is_newer("0.5.9", "0.6.0"));
        assert!(!is_newer("garbage", "0.6.0"));
    }

    #[test]
    fn tag_from_release_json_extracts() {
        let body = r#"{"tag_name":"v0.6.0","name":"Release v0.6.0"}"#;
        assert_eq!(tag_from_release_json(body).as_deref(), Some("v0.6.0"));
        assert_eq!(tag_from_release_json("{}"), None);
    }

    #[test]
    fn newest_tag_picks_max_by_semver_not_list_order() {
        let body = r#"[{"name":"v0.5.0"},{"name":"v0.6.0"},{"name":"v0.5.9"},{"name":"nightly"}]"#;
        assert_eq!(newest_tag_from_tags_json(body).as_deref(), Some("v0.6.0"));
        assert_eq!(newest_tag_from_tags_json("[]"), None);
    }

    #[test]
    fn install_prefix_from_bin_layout() {
        assert_eq!(
            install_prefix(Path::new("/home/u/.local/bin/agent-cli")),
            Some(PathBuf::from("/home/u/.local"))
        );
    }

    #[test]
    fn is_branch_ref_distinguishes_tag_from_branch() {
        assert!(is_branch_ref("main"));
        assert!(is_branch_ref("feat/x"));
        assert!(!is_branch_ref("v0.6.0"));
        assert!(!is_branch_ref("0.6.0"));
    }
}
