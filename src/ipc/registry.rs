use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::id::AgentId;
use crate::persona::PersonaSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: AgentId,
    pub name: Option<String>,
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
            // stale エントリは掃除
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
