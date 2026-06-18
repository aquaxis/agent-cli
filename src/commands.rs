use std::time::Duration;

use crate::ai;
use crate::config::{Config, ConfigSource};
use crate::error::{AppError, Result};
use crate::id::AgentId;
use crate::ipc::{client, registry, IpcMessage};

pub async fn list(cfg: &Config) -> Result<()> {
    let dir = cfg.registry_dir()?;
    let entries = registry::list_entries(&dir)?;
    if entries.is_empty() {
        println!("no agents running.");
        return Ok(());
    }
    let rows: Vec<[String; 6]> = entries
        .into_iter()
        .map(|e| {
            let role = e
                .persona
                .as_ref()
                .map(|p| p.role.clone())
                .unwrap_or_default();
            let skills = e
                .persona
                .as_ref()
                .map(|p| p.skills.join(", "))
                .unwrap_or_default();
            [
                e.id.to_string(),
                e.name.clone().unwrap_or_else(|| "-".into()),
                e.provider,
                e.model,
                role,
                skills,
            ]
        })
        .collect();
    let headers = ["ID", "NAME", "PROVIDER", "MODEL", "ROLE", "SKILLS"];
    let mut widths = headers.map(|s| s.len());
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.chars().count() > widths[i] {
                widths[i] = cell.chars().count();
            }
        }
    }
    let render = |cells: &[String; 6]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:<width$}", c, width = widths[i]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    let header_row: [String; 6] = headers.map(|s| s.to_string());
    println!("{}", render(&header_row));
    for row in &rows {
        println!("{}", render(row));
    }
    Ok(())
}

pub async fn send(cfg: &Config, peer: &str, text: &str) -> Result<()> {
    let dir = cfg.registry_dir()?;
    let target = registry::resolve_peer(&dir, peer)?;
    let me = AgentId::new();
    let msg = IpcMessage::Prompt {
        from: me,
        from_name: Some("cli-send".into()),
        text: text.to_string(),
        reply_to: None,
    };
    match client::send(&target.socket, &msg).await? {
        IpcMessage::Ack { .. } => {
            println!("delivered to {}", target.id);
            Ok(())
        }
        IpcMessage::Error { message } => Err(AppError::ipc(message)),
        other => Err(AppError::ipc(format!("unexpected response: {:?}", other))),
    }
}

pub async fn ask_and_receive(
    cfg: &Config,
    peer: &str,
    text: &str,
    timeout_secs: u64,
) -> Result<String> {
    use crate::ipc::server::IpcServer;

    let dir = cfg.registry_dir()?;
    let target = registry::resolve_peer(&dir, peer)?;
    let me = AgentId::new();

    let reply_dir = tempfile::tempdir()?;
    let reply_socket = reply_dir.path().join("reply.sock");
    let mut reply_server = IpcServer::bind(reply_socket.clone()).await?;
    let mut reply_rx = reply_server.take_rx().expect("rx available after bind");

    let msg = IpcMessage::Prompt {
        from: me,
        from_name: Some("cli-ask".into()),
        text: text.to_string(),
        reply_to: Some(reply_socket.clone()),
    };
    match client::send(&target.socket, &msg).await? {
        IpcMessage::Ack { .. } => {}
        IpcMessage::Error { message } => return Err(AppError::ipc(message)),
        other => return Err(AppError::ipc(format!("unexpected response: {:?}", other))),
    }

    let result = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        reply_rx.recv(),
    )
    .await;

    drop(reply_server);
    drop(reply_dir);

    match result {
        Ok(Some(IpcMessage::PromptReply { text, .. })) => Ok(text),
        Ok(Some(other)) => Err(AppError::ipc(format!("unexpected reply: {:?}", other))),
        Ok(None) => Err(AppError::ipc("reply channel closed without response")),
        Err(_) => Err(AppError::ipc(format!(
            "timed out waiting for reply after {timeout_secs}s"
        ))),
    }
}

pub async fn ask(cfg: &Config, peer: &str, text: &str, timeout_secs: u64) -> Result<()> {
    let response = ask_and_receive(cfg, peer, text, timeout_secs).await?;
    println!("{}", response);
    Ok(())
}

pub async fn providers(cfg: &Config) -> Result<()> {
    println!("Active provider: {}", cfg.provider.kind);
    println!();
    println!("Supported backends:");
    for kind in ai::SUPPORTED {
        let entry = cfg.provider_entry(kind);
        let model = entry
            .and_then(|e| e.model.clone())
            .unwrap_or_else(|| "-".into());
        let key_status = match (*kind, entry) {
            ("claude", Some(e)) | ("codex", Some(e)) => match &e.api_key_env {
                Some(env) => match std::env::var(env) {
                    Ok(_) => format!("env {env}: set"),
                    Err(_) => format!("env {env}: NOT set"),
                },
                None => "no api_key_env configured".into(),
            },
            ("ollama", Some(_)) | ("llama.cpp", Some(_)) => "local server".into(),
            (_, _) => "no config".into(),
        };
        println!("  - {kind:<12} model={model:<20} {key_status}");
    }
    Ok(())
}

pub async fn doctor(cfg: &mut Config, source: &ConfigSource) -> Result<()> {
    let mut all_ok = true;
    println!("[doctor] config path     : {}", source.path.display());
    println!("[doctor] config explicit : {}", source.from_explicit);

    // Provider check (before normalization so opencode-go appears as-is)
    let kind = cfg.provider.kind.as_str();
    print!("[doctor] provider kind   : {kind} ... ");
    match ai::SUPPORTED.contains(&kind) {
        true => println!("OK"),
        false => {
            println!("UNKNOWN provider");
            all_ok = false;
        }
    }

    // Normalize opencode-go → opencode with Go defaults before further checks
    cfg.apply_opencode_go_defaults();

    // API key check (if applicable)
    let kind = cfg.provider.kind.as_str();
    if let Some(entry) = cfg.provider_entry(kind) {
        if let Some(env) = &entry.api_key_env {
            print!("[doctor] api key env     : {env} ... ");
            match std::env::var(env) {
                Ok(_) => println!("set"),
                Err(_) => {
                    println!("NOT set");
                    if matches!(kind, "claude" | "codex") {
                        all_ok = false;
                    }
                }
            }
        }
    }

    // Registry dir
    let reg_dir = cfg.registry_dir()?;
    print!("[doctor] registry dir    : {} ... ", reg_dir.display());
    match std::fs::create_dir_all(&reg_dir) {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("ERROR ({e})");
            all_ok = false;
        }
    }

    // Log dir
    let log_dir = cfg.log_dir()?;
    print!("[doctor] log dir         : {} ... ", log_dir.display());
    match std::fs::create_dir_all(&log_dir) {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("ERROR ({e})");
            all_ok = false;
        }
    }

    // Bash check
    print!("[doctor] bash            : ");
    match tokio::process::Command::new("bash")
        .arg("-c")
        .arg("echo agent-cli")
        .output()
        .await
    {
        Ok(o) if o.status.success() => println!("OK"),
        Ok(o) => {
            println!(
                "FAIL (exit={:?}, stderr={})",
                o.status.code(),
                String::from_utf8_lossy(&o.stderr)
            );
            all_ok = false;
        }
        Err(e) => {
            println!("FAIL ({e})");
            all_ok = false;
        }
    }

    // Provider connectivity (best-effort).
    // Cloud routing models (e.g. ollama's `*:cloud` tag) can take tens of seconds on cold start,
    // so we use the same 60-second timeout as selftest Stage 1.
    println!("[doctor] provider conn   :");
    let provider = match ai::build(cfg, source) {
        Ok(p) => p,
        Err(e) => {
            print_provider_error("[doctor]   ", &e);
            all_ok = false;
            return finish(all_ok);
        }
    };
    let messages = vec![ai::Message::User {
        content: "ping".into(),
    }];
    let conn = tokio::time::timeout(
        Duration::from_secs(60),
        provider.complete_stream(&messages, &[]),
    )
    .await;
    match conn {
        Ok(Ok(_)) => println!("[doctor]   OK (stream initiated)"),
        Ok(Err(e)) => {
            print_provider_error("[doctor]   ", &e);
            all_ok = false;
        }
        Err(_) => {
            println!("[doctor]   TIMEOUT (>60s)");
            all_ok = false;
        }
    }

    finish(all_ok)
}

/// Print multi-line details of `AppError::Provider` to stdout with indentation (FR-09-3).
/// Since `ProviderError::Display` returns a string containing newlines, each line gets a prefix.
fn print_provider_error(indent: &str, e: &AppError) {
    let s = e.to_string();
    let mut iter = s.lines();
    if let Some(first) = iter.next() {
        println!("{indent}FAIL: {first}");
    }
    for line in iter {
        println!("{indent}  {line}");
    }
}

fn finish(ok: bool) -> Result<()> {
    if ok {
        println!("[doctor] result          : OK");
        Ok(())
    } else {
        println!("[doctor] result          : FAIL");
        Err(AppError::Other("doctor reported failures".into()))
    }
}

pub async fn selftest(
    cfg: &Config,
    source: &ConfigSource,
    provider_override: Option<&str>,
) -> Result<()> {
    let mut cfg = cfg.clone();
    if let Some(p) = provider_override {
        cfg.apply_overrides(Some(p), None);
    }
    cfg.apply_opencode_go_defaults();
    let mut all_ok = true;
    let mut stage1_ok = false;

    println!("[selftest] stage 1 (provider OK round-trip)");
    match stage_provider_ok(&mut cfg, source).await {
        Ok(()) => stage1_ok = true,
        Err(e) => {
            print_provider_error("[selftest]   ", &e);
            all_ok = false;
        }
    }

    println!("[selftest] stage 2 (tool execution: shell)");
    if let Err(e) = stage_shell_tool(&cfg).await {
        println!("[selftest]   FAIL ({e})");
        all_ok = false;
    }

    println!("[selftest] stage 3 (IPC round-trip)");
    if let Err(e) = stage_ipc_roundtrip().await {
        println!("[selftest]   FAIL ({e})");
        all_ok = false;
    }

    println!("[selftest] stage 4 (subprocess registration + IPC)");
    if let Err(e) = stage_subprocess_ipc().await {
        println!("[selftest]   FAIL ({e})");
        all_ok = false;
    }

    println!("[selftest] stage 5 (subprocess peer prompt + AI response)");
    if !stage1_ok {
        println!("[selftest]   SKIP (stage 1 failed, provider unreachable)");
    } else if let Err(e) = stage_subprocess_ai_response(&cfg).await {
        println!("[selftest]   FAIL ({e})");
        all_ok = false;
    }

    if all_ok {
        println!("[selftest] result  : OK");
        Ok(())
    } else {
        println!("[selftest] result  : FAIL");
        Err(AppError::Other("selftest reported failures".into()))
    }
}

async fn stage_provider_ok(cfg: &mut Config, source: &ConfigSource) -> Result<()> {
    use futures::StreamExt;
    let provider = ai::build(cfg, source)?;
    let messages = vec![
        ai::Message::System {
            content: "You are a tester. Respond with exactly the literal text OK.".into(),
        },
        ai::Message::User {
            content: "Reply with the literal text OK.".into(),
        },
    ];
    let mut stream = provider.complete_stream(&messages, &[]).await?;
    let mut buf = String::new();
    let timed = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(ev) = stream.next().await {
            match ev {
                ai::ProviderEvent::Text { delta } => buf.push_str(&delta),
                ai::ProviderEvent::Done => break,
                ai::ProviderEvent::Error { message } => {
                    return Err(AppError::provider(provider.name(), message));
                }
                _ => {}
            }
        }
        Ok::<_, AppError>(())
    })
    .await;
    match timed {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            return Err(AppError::provider(provider.name(), "selftest timed out"));
        }
    }
    println!(
        "[selftest]   provider: {} model={}",
        provider.name(),
        provider.model()
    );
    println!("[selftest]   response: {}", buf.trim());
    if !buf.to_uppercase().contains("OK") {
        return Err(AppError::Other("response did not contain OK token".into()));
    }
    println!("[selftest]   stage 1 ok");
    Ok(())
}

async fn stage_shell_tool(cfg: &Config) -> Result<()> {
    use crate::tools::{ToolCtx, ToolRegistry};
    let tools = ToolRegistry::build(cfg, None, None);
    let tool = tools
        .get("shell")
        .ok_or_else(|| AppError::Other("shell tool is not enabled".into()))?;
    let ctx = ToolCtx {
        self_id: crate::id::AgentId::new(),
        registry_dir: std::path::PathBuf::from("/tmp/agent-cli-selftest-noop"),
    };
    let out = tool
        .invoke(serde_json::json!({"cmd": "echo selftest"}), &ctx)
        .await?;
    if !out.ok {
        return Err(AppError::Other(format!(
            "shell tool returned failure: {}",
            out.content
        )));
    }
    if !out.content.contains("selftest") {
        return Err(AppError::Other(format!(
            "shell tool did not echo selftest: {}",
            out.content
        )));
    }
    println!("[selftest]   stage 2 ok (shell tool executed)");
    Ok(())
}

async fn stage_ipc_roundtrip() -> Result<()> {
    use crate::ipc::{client, server::IpcServer, IpcMessage};
    use tempfile::TempDir;
    let dir = TempDir::new().map_err(|e| AppError::Other(e.to_string()))?;
    let path = dir.path().join("selftest.sock");
    let _server = IpcServer::bind(path.clone()).await?;
    let resp = client::send(&path, &IpcMessage::Ping).await?;
    match resp {
        IpcMessage::Pong => {
            println!("[selftest]   stage 3 ok (Ping/Pong)");
            Ok(())
        }
        other => Err(AppError::Other(format!(
            "unexpected IPC response: {:?}",
            other
        ))),
    }
}

async fn stage_subprocess_ipc() -> Result<()> {
    use crate::ipc::{client, registry, IpcMessage};
    use std::process::Stdio;
    use tempfile::TempDir;

    let exe = std::env::current_exe().map_err(|e| AppError::Other(format!("current_exe: {e}")))?;
    let dir = TempDir::new().map_err(|e| AppError::Other(e.to_string()))?;
    let cfg_path = dir.path().join("child.toml");
    let registry_dir = dir.path().join("reg");
    let log_dir = dir.path().join("log");
    let agents_dir = dir.path().join("agents");
    std::fs::create_dir_all(&registry_dir)?;
    std::fs::create_dir_all(&log_dir)?;
    std::fs::create_dir_all(&agents_dir)?;
    // Use ollama with an unreachable base_url so the child does not call external APIs.
    let toml = format!(
        r#"[provider]
kind = "ollama"

[provider.ollama]
model = "selftest"
base_url = "http://127.0.0.1:65535"

[runtime]
auto_approve_tools = true
log_dir = {log_dir:?}
registry_dir = {registry_dir:?}
agents_dir = {agents_dir:?}

[tools]
enabled = []
"#,
        log_dir = log_dir.display().to_string(),
        registry_dir = registry_dir.display().to_string(),
        agents_dir = agents_dir.display().to_string(),
    );
    std::fs::write(&cfg_path, toml)?;

    let mut child = tokio::process::Command::new(&exe)
        .arg("--config")
        .arg(&cfg_path)
        .arg("run")
        .arg("--name")
        .arg("selftest-child")
        .arg("--auto-approve-tools")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn child: {e}")))?;

    // Wait for child to register (up to 5 seconds)
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let entry = loop {
        let entries = registry::list_entries(&registry_dir).unwrap_or_default();
        if let Some(e) = entries
            .into_iter()
            .find(|e| e.name.as_deref() == Some("selftest-child"))
        {
            break Some(e);
        }
        if std::time::Instant::now() >= deadline {
            break None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let entry = match entry {
        Some(e) => e,
        None => {
            let _ = child.kill().await;
            return Err(AppError::Other(
                "child agent did not register within 5s".into(),
            ));
        }
    };

    // Ping/Pong
    let ping_resp = client::send(&entry.socket, &IpcMessage::Ping).await;
    // Prompt/Ack
    let prompt_resp = client::send(
        &entry.socket,
        &IpcMessage::Prompt {
            from: crate::id::AgentId::new(),
            from_name: Some("selftest-driver".into()),
            text: "selftest-prompt".into(),
            reply_to: None,
        },
    )
    .await;

    // Child termination: closing stdin causes EOF, which breaks the REPL loop.
    // Regression test for FR-13 "App termination": verify the child exits on its own.
    drop(child.stdin.take());
    let exited_naturally = tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .is_ok();
    // Kill just in case (harmless if already exited)
    let _ = child.kill().await;

    let ping_resp = ping_resp.map_err(|e| AppError::Other(format!("ipc send (Ping): {e}")))?;
    if !matches!(ping_resp, IpcMessage::Pong) {
        return Err(AppError::Other(format!(
            "unexpected IPC response (Ping): {:?}",
            ping_resp
        )));
    }
    let prompt_resp =
        prompt_resp.map_err(|e| AppError::Other(format!("ipc send (Prompt): {e}")))?;
    if !matches!(prompt_resp, IpcMessage::Ack { .. }) {
        return Err(AppError::Other(format!(
            "unexpected IPC response (Prompt): {:?}",
            prompt_resp
        )));
    }
    if !exited_naturally {
        return Err(AppError::Other(
            "child agent did not exit on stdin EOF within 3s (FR-13)".into(),
        ));
    }
    // Verify socket and registry meta are cleaned up after exit
    if entry.socket.exists() {
        return Err(AppError::Other(format!(
            "socket file remained after exit: {}",
            entry.socket.display()
        )));
    }
    let meta_path = registry_dir.join(format!("{}.json", entry.id.as_str()));
    if meta_path.exists() {
        return Err(AppError::Other(format!(
            "registry meta file remained after exit: {}",
            meta_path.display()
        )));
    }
    println!(
        "[selftest]   stage 4 ok (subprocess {} registered, Ping/Pong + Prompt/Ack, EOF clean exit)",
        entry.id
    );
    Ok(())
}

async fn stage_subprocess_ai_response(cfg: &Config) -> Result<()> {
    use crate::ipc::{client, registry, IpcMessage};
    use std::process::Stdio;
    use tempfile::TempDir;

    let exe = std::env::current_exe().map_err(|e| AppError::Other(format!("current_exe: {e}")))?;
    let dir = TempDir::new().map_err(|e| AppError::Other(e.to_string()))?;

    // Copy the original cfg, replacing only registry_dir / log_dir / agents_dir with temp paths.
    // Provider settings (base_url, model, API key env) are kept as-is so the child calls the real LLM.
    let mut child_cfg = cfg.clone();
    let registry_dir = dir.path().join("reg");
    let log_dir = dir.path().join("log");
    let agents_dir = dir.path().join("agents");
    std::fs::create_dir_all(&registry_dir)?;
    std::fs::create_dir_all(&log_dir)?;
    std::fs::create_dir_all(&agents_dir)?;
    child_cfg.runtime.registry_dir = registry_dir.display().to_string();
    child_cfg.runtime.log_dir = log_dir.display().to_string();
    child_cfg.runtime.agents_dir = agents_dir.display().to_string();
    child_cfg.runtime.persona_file = String::new();
    child_cfg.runtime.auto_approve_tools = true;
    // Tools are unnecessary (the AI just needs to respond to the peer prompt with text)
    child_cfg.tools.enabled = Vec::new();

    let cfg_path = dir.path().join("child.toml");
    let toml_str = toml::to_string_pretty(&child_cfg)
        .map_err(|e| AppError::Other(format!("serialize child config: {e}")))?;
    std::fs::write(&cfg_path, toml_str)?;

    let mut child = tokio::process::Command::new(&exe)
        .arg("--config")
        .arg(&cfg_path)
        .arg("run")
        .arg("--name")
        .arg("selftest-peer")
        .arg("--auto-approve-tools")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Other(format!("spawn child: {e}")))?;

    // Wait for child to register
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let entry = loop {
        let entries = registry::list_entries(&registry_dir).unwrap_or_default();
        if let Some(e) = entries
            .into_iter()
            .find(|e| e.name.as_deref() == Some("selftest-peer"))
        {
            break Some(e);
        }
        if std::time::Instant::now() >= deadline {
            break None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let entry = match entry {
        Some(e) => e,
        None => {
            let _ = child.kill().await;
            return Err(AppError::Other(
                "child agent did not register within 10s".into(),
            ));
        }
    };

    // Send prompt (a short instruction likely to produce a response)
    let prompt_msg = IpcMessage::Prompt {
        from: crate::id::AgentId::new(),
        from_name: Some("selftest-driver".into()),
        text: "Reply with a single word: HELLO".into(),
        reply_to: None,
    };
    let send_resp = client::send(&entry.socket, &prompt_msg).await;

    // Wait for an assistant entry in the child's log directory (up to 90 seconds)
    let agent_log_dir = log_dir.join(entry.id.as_str());
    let observed = wait_for_assistant_log(&agent_log_dir, Duration::from_secs(90)).await;

    // Cleanup
    drop(child.stdin.take());
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    let _ = child.kill().await;

    let send_resp = send_resp.map_err(|e| AppError::Other(format!("send Prompt: {e}")))?;
    if !matches!(send_resp, IpcMessage::Ack { .. }) {
        return Err(AppError::Other(format!(
            "unexpected response to Prompt: {:?}",
            send_resp
        )));
    }
    match observed {
        Some(text) => {
            let preview: String = text.chars().take(80).collect();
            println!(
                "[selftest]   stage 5 ok (peer responded: \"{preview}{}\")",
                if text.chars().count() > 80 { "..." } else { "" }
            );
            Ok(())
        }
        None => Err(AppError::Other(
            "no assistant response observed within 90s".into(),
        )),
    }
}

/// Watch the specified agent log directory and return the text of the first JSON Lines
/// entry with `kind=assistant`. Returns `None` on timeout.
async fn wait_for_assistant_log(dir: &std::path::Path, timeout: Duration) -> Option<String> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if let Ok(read_dir) = std::fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                            if v.get("kind").and_then(|k| k.as_str()) == Some("assistant") {
                                if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                                    if !text.trim().is_empty() {
                                        return Some(text.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    None
}

pub fn config_show(cfg: &Config) -> Result<()> {
    let s = toml::to_string_pretty(cfg)?;
    println!("{s}");
    Ok(())
}

pub fn config_edit(source: &ConfigSource) -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new(editor)
        .arg(&source.path)
        .status()?;
    if !status.success() {
        return Err(AppError::Other("editor exited with non-zero status".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::server::IpcServer;
    use crate::ipc::IpcMessage;
    use crate::ipc::registry::RegistryEntry;
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Write a registry entry JSON file manually (without RegistryHandle, so it
    /// is not cleaned up by Drop when the setup function returns).
    fn write_registry_entry(dir: &PathBuf, entry: &RegistryEntry) {
        let meta_path = dir.join(format!("{}.json", entry.id.as_str()));
        let raw = serde_json::to_vec_pretty(entry).unwrap();
        std::fs::write(&meta_path, raw).unwrap();
    }

    /// Create a mock peer that receives a Prompt and sends a PromptReply
    /// back to the specified reply_to socket.
    async fn setup_mock_peer(
        registry_dir: &PathBuf,
        name: &str,
        reply_text: &str,
    ) -> RegistryEntry {
        let id = AgentId::new();
        let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));
        let mut server = IpcServer::bind(socket_path.clone()).await.unwrap();
        let mut rx = server.take_rx().unwrap();

        let reply_text = reply_text.to_string();
        tokio::spawn(async move {
            let _server = server;
            if let Some(msg) = rx.recv().await {
                if let IpcMessage::Prompt {
                    reply_to: Some(reply_socket),
                    ..
                } = msg
                {
                    let reply = IpcMessage::PromptReply {
                        from: AgentId::new(),
                        text: reply_text,
                    };
                    let _ = client::send(&reply_socket, &reply).await;
                }
            }
        });

        let entry = RegistryEntry {
            id: id.clone(),
            name: Some(name.into()),
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
        };
        write_registry_entry(registry_dir, &entry);
        entry
    }

    #[tokio::test]
    async fn ask_and_receive_with_mock_peer() {
        let dir = TempDir::new().unwrap();
        let registry_dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&registry_dir).unwrap();

        let _entry = setup_mock_peer(&registry_dir, "mock-ask-peer", "HELLO from mock").await;

        // Build a minimal config pointing to the temp registry dir
        let cfg: Config = toml::from_str(&format!(
            r#"[provider]
kind = "ollama"

[provider.ollama]
model = "test"

[runtime]
registry_dir = {:?}
"#,
            registry_dir.display().to_string()
        ))
        .unwrap();

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            ask_and_receive(&cfg, "mock-ask-peer", "Reply with HELLO", 30),
        )
        .await
        .expect("ask_and_receive timeout");

        assert!(result.is_ok(), "ask_and_receive should succeed: {:?}", result.err());
        assert_eq!(result.unwrap(), "HELLO from mock");
    }

    #[tokio::test]
    async fn ask_and_receive_times_out() {
        let dir = TempDir::new().unwrap();
        let registry_dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&registry_dir).unwrap();

        // Create a mock peer that does NOT send a reply
        let id = AgentId::new();
        let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));
        let mut server = IpcServer::bind(socket_path.clone()).await.unwrap();
        let mut rx = server.take_rx().unwrap();
        tokio::spawn(async move {
            let _server = server;
            // Consume the message but don't reply
            let _ = rx.recv().await;
        });

        let entry = RegistryEntry {
            id: id.clone(),
            name: Some("silent-peer".into()),
            pid: std::process::id(),
            started_at: Utc::now(),
            provider: "mock".into(),
            model: "mock".into(),
            socket: socket_path,
            persona: None,
        };
        write_registry_entry(&registry_dir, &entry);

        let cfg: Config = toml::from_str(&format!(
            r#"[provider]
kind = "ollama"

[provider.ollama]
model = "test"

[runtime]
registry_dir = {:?}
"#,
            registry_dir.display().to_string()
        ))
        .unwrap();

        // Use a 1-second timeout — should time out
        let result = ask_and_receive(&cfg, "silent-peer", "hello", 1).await;

        assert!(result.is_err(), "ask_and_receive should time out");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("timed out"),
            "error should mention timeout, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn ask_and_receive_peer_not_found() {
        let dir = TempDir::new().unwrap();
        let registry_dir = dir.path().to_path_buf();
        std::fs::create_dir_all(&registry_dir).unwrap();

        let cfg: Config = toml::from_str(&format!(
            r#"[provider]
kind = "ollama"

[provider.ollama]
model = "test"

[runtime]
registry_dir = {:?}
"#,
            registry_dir.display().to_string()
        ))
        .unwrap();

        let result = ask_and_receive(&cfg, "nonexistent-peer", "hello", 5).await;
        assert!(result.is_err(), "should error for non-existent peer");
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "error should mention peer not found"
        );
    }
}
