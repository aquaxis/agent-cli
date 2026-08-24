//! End-to-end test for the detached-spawn feature (FR-04 – FR-11, AC-01 – AC-04).
//!
//! Verifies that `agent-cli spawn` creates a headless agent that outlives the
//! launching process, and that `agent-cli stop` terminates it and cleans up its
//! registry entry. Uses an unreachable ollama base_url so no network call is
//! made — the agent only registers and idles until stopped (the same approach
//! as the `selftest` subprocess stages).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_agent-cli")
}

fn proc_alive(pid: u64) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Return the parsed registry JSON for the entry whose `name` matches, if any.
fn find_entry(registry_dir: &Path, name: &str) -> Option<serde_json::Value> {
    let rd = std::fs::read_dir(registry_dir).ok()?;
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("name").and_then(|n| n.as_str()) == Some(name) {
            return Some(v);
        }
    }
    None
}

fn write_config(dir: &Path) -> (PathBuf, PathBuf) {
    let registry_dir = dir.join("reg");
    let log_dir = dir.join("log");
    let agents_dir = dir.join("agents");
    for d in [&registry_dir, &log_dir, &agents_dir] {
        std::fs::create_dir_all(d).unwrap();
    }
    let cfg_path = dir.join("config.toml");
    let toml = format!(
        r#"[provider]
kind = "ollama"

[provider.ollama]
model = "test"
base_url = "http://127.0.0.1:65535"

[runtime]
auto_approve_tools = true
log_dir = {log:?}
registry_dir = {reg:?}
agents_dir = {agents:?}

[tools]
enabled = []
"#,
        log = log_dir.display().to_string(),
        reg = registry_dir.display().to_string(),
        agents = agents_dir.display().to_string(),
    );
    std::fs::write(&cfg_path, toml).unwrap();
    (cfg_path, registry_dir)
}

#[test]
fn spawn_detached_agent_outlives_launcher_then_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let (cfg_path, registry_dir) = write_config(tmp.path());

    // Launch a detached agent. `spawn` returns after the child self-registers;
    // the child keeps running in its own session (FR-04 – FR-07).
    let out = Command::new(bin())
        .arg("--config")
        .arg(&cfg_path)
        .arg("spawn")
        .arg("--name")
        .arg("testworker")
        .output()
        .expect("run spawn");
    assert!(
        out.status.success(),
        "spawn failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The launcher has exited, but the detached child must be registered + alive
    // (AC-01, AC-02).
    let entry = find_entry(&registry_dir, "testworker").expect("child should be registered");
    let pid = entry
        .get("pid")
        .and_then(|p| p.as_u64())
        .expect("registry entry has pid");
    assert!(
        proc_alive(pid),
        "detached child (pid {pid}) should still be alive after the launcher exited"
    );

    // Stop it (AC-03).
    let out = Command::new(bin())
        .arg("--config")
        .arg(&cfg_path)
        .arg("stop")
        .arg("testworker")
        .output()
        .expect("run stop");
    assert!(
        out.status.success(),
        "stop failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Within a few seconds the process exits and its registry entry is gone.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let gone = !proc_alive(pid) && find_entry(&registry_dir, "testworker").is_none();
        if gone {
            break;
        }
        if Instant::now() >= deadline {
            // Best-effort cleanup so a failed run leaves no stray process.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            panic!("detached agent (pid {pid}) did not stop within 5s");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
