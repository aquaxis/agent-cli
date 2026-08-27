use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::id::{AgentId, GroupId};
use crate::persona::PersonaSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: AgentId,
    pub name: Option<String>,
    /// Group this agent belongs to (a cohort launched together). Absent for
    /// ungrouped agents; `#[serde(default)]` keeps registry files written by
    /// older versions (which have no `group` key) loadable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    pub provider: String,
    pub model: String,
    pub socket: PathBuf,
    pub persona: Option<PersonaSummary>,
}

pub struct RegistryHandle {
    #[allow(dead_code)]
    pub dir: PathBuf,
    pub meta_path: PathBuf,
    #[allow(dead_code)]
    pub socket_path: PathBuf,
}

impl RegistryHandle {
    pub async fn register(dir: &Path, entry: &RegistryEntry) -> Result<Self> {
        tokio::fs::create_dir_all(dir).await?;
        // 0700 permission for the registry dir
        if let Ok(meta) = std::fs::metadata(dir) {
            let mut perm = meta.permissions();
            perm.set_mode(0o700);
            let _ = std::fs::set_permissions(dir, perm);
        }
        let meta_path = dir.join(format!("{}.json", entry.id.as_str()));
        let socket_path = dir.join(format!("{}.sock", entry.id.as_str()));
        let raw = serde_json::to_vec_pretty(entry)?;
        tokio::fs::write(&meta_path, raw).await?;
        Ok(Self {
            dir: dir.to_path_buf(),
            meta_path,
            socket_path,
        })
    }

    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.meta_path);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for RegistryHandle {
    /// As a guarantee of FR-13 "App termination", ensures registry metadata and
    /// socket are removed regardless of how `run` completes (normal exit or panic).
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.meta_path);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

pub fn list_entries(dir: &Path) -> Result<Vec<RegistryEntry>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let parsed: RegistryEntry = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !parsed.socket.exists() {
            // Clean up stale entries
            let _ = std::fs::remove_file(&path);
            continue;
        }
        if !pid_alive(parsed.pid) {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&parsed.socket);
            continue;
        }
        out.push(parsed);
    }
    Ok(out)
}

fn pid_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

pub fn resolve_peer(dir: &Path, key: &str) -> Result<RegistryEntry> {
    let entries = list_entries(dir)?;
    for e in &entries {
        if e.id.as_str() == key {
            return Ok(e.clone());
        }
    }
    for e in &entries {
        if e.name.as_deref() == Some(key) {
            return Ok(e.clone());
        }
    }
    Err(AppError::registry(format!(
        "peer not found by id or name: {key}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(group: Option<GroupId>) -> RegistryEntry {
        RegistryEntry {
            id: AgentId::new(),
            name: Some("a".into()),
            group,
            pid: 1,
            started_at: Utc::now(),
            provider: "ollama".into(),
            model: "m".into(),
            socket: PathBuf::from("/tmp/a.sock"),
            persona: None,
        }
    }

    #[test]
    fn entry_roundtrips_with_group() {
        let entry = sample(Some(GroupId("team".into())));
        let raw = serde_json::to_string(&entry).unwrap();
        assert!(raw.contains("\"group\""));
        let back: RegistryEntry = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.group.as_ref().map(GroupId::as_str), Some("team"));
    }

    #[test]
    fn ungrouped_entry_omits_group_key() {
        let entry = sample(None);
        let raw = serde_json::to_string(&entry).unwrap();
        assert!(!raw.contains("\"group\""));
    }

    #[test]
    fn legacy_entry_without_group_loads() {
        // A registry JSON written before the group feature has no `group` key.
        let legacy = r#"{
            "id": "agent-01",
            "name": "old",
            "pid": 1,
            "started_at": "2026-01-01T00:00:00Z",
            "provider": "ollama",
            "model": "m",
            "socket": "/tmp/old.sock",
            "persona": null
        }"#;
        let parsed: RegistryEntry = serde_json::from_str(legacy).unwrap();
        assert!(parsed.group.is_none());
    }
}
