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
    /// The agents that created this one, root first, excluding itself: the
    /// last element is the direct parent and the length is the depth. Empty for
    /// an agent a human started — which is every agent in a registry written
    /// before lineage existed, hence the serde default and the empty-skip, so
    /// such an entry serialises exactly as it always did.
    ///
    /// The whole chain is kept rather than just the parent because an agent in
    /// the middle can exit: its children keep running, and they are still the
    /// responsibility of whoever started the chain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ancestors: Vec<AgentId>,
}

impl RegistryEntry {
    /// The agent that directly created this one.
    pub fn parent(&self) -> Option<&AgentId> {
        self.ancestors.last()
    }

    /// Depth of the spawn chain: 0 for an agent nobody spawned.
    pub fn depth(&self) -> u32 {
        self.ancestors.len() as u32
    }

    /// Whether `id` created this agent, directly or through others.
    pub fn is_descendant_of(&self, id: &AgentId) -> bool {
        self.ancestors.iter().any(|a| a.as_str() == id.as_str())
    }

    /// How many levels below `id` this agent sits (1 = a direct child), or
    /// `None` when it is not in `id`'s tree.
    pub fn distance_from(&self, id: &AgentId) -> Option<u32> {
        let pos = self
            .ancestors
            .iter()
            .position(|a| a.as_str() == id.as_str())?;
        Some((self.ancestors.len() - pos) as u32)
    }
}

/// Environment variable the launcher sets on a peer it spawns: the peer's
/// ancestor chain, root first, comma-separated. It is set on the spawned
/// process only — the command line is untouched, which keeps every existing
/// subcommand's behaviour exactly as it was.
pub const ENV_ANCESTORS: &str = "AGENT_CLI_ANCESTORS";

/// This agent's own ancestor chain, read once at startup. Unset, empty or
/// unparsable means "nobody spawned me", which is what a directly started agent
/// is; unparsable elements are dropped rather than failing the startup.
pub fn ancestors_from_env() -> Vec<AgentId> {
    std::env::var(ENV_ANCESTORS)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<AgentId>().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Render a chain for [`ENV_ANCESTORS`].
pub fn ancestors_to_env(chain: &[AgentId]) -> String {
    chain
        .iter()
        .map(|a| a.as_str())
        .collect::<Vec<_>>()
        .join(",")
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

/// Live agents whose parent is `id` — this agent's direct children. Dead peers
/// are already gone: `list_entries` prunes as it reads.
pub fn children_of(dir: &Path, id: &AgentId) -> Result<Vec<RegistryEntry>> {
    Ok(list_entries(dir)?
        .into_iter()
        .filter(|e| e.parent().map(|p| p.as_str()) == Some(id.as_str()))
        .collect())
}

/// Live agents in `id`'s subtree, nearest first, each with its distance from
/// `id` (1 = a direct child).
///
/// Because every entry carries its whole chain, this is a filter rather than a
/// walk: an agent in the middle of the chain may have exited, and its children
/// are still found — they are still `id`'s to manage.
pub fn descendants_of(dir: &Path, id: &AgentId) -> Result<Vec<(RegistryEntry, u32)>> {
    let mut out: Vec<(RegistryEntry, u32)> = list_entries(dir)?
        .into_iter()
        .filter_map(|e| e.distance_from(id).map(|d| (e, d)))
        .collect();
    out.sort_by_key(|(_, d)| *d);
    Ok(out)
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
            ancestors: Vec::new(),
        }
    }

    /// Lineage survives a round trip, and an agent nobody spawned writes the
    /// same JSON it wrote before lineage existed.
    #[test]
    fn lineage_roundtrips_and_stays_out_of_an_unparented_entry() {
        let root = AgentId::new();
        let mut entry = sample(None);
        entry.ancestors = vec![root.clone()];
        let raw = serde_json::to_string(&entry).unwrap();
        let back: RegistryEntry = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.parent().map(AgentId::as_str), Some(root.as_str()));
        assert_eq!(back.depth(), 1);

        let unparented = serde_json::to_string(&sample(None)).unwrap();
        assert!(
            !unparented.contains("ancestors"),
            "an agent a human started must serialise as it always did: {unparented}"
        );
    }

    /// A registry file written before lineage existed loads as a root.
    #[test]
    fn an_entry_without_ancestors_loads_as_a_root() {
        let old = r#"{
            "id": "agent-01ABC",
            "name": "old",
            "pid": 1,
            "started_at": "2026-01-01T00:00:00Z",
            "provider": "ollama",
            "model": "m",
            "socket": "/tmp/old.sock",
            "persona": null
        }"#;
        let entry: RegistryEntry = serde_json::from_str(old).expect("older files must still load");
        assert!(entry.ancestors.is_empty());
        assert_eq!(entry.depth(), 0);
        assert!(entry.parent().is_none());
    }

    #[test]
    fn the_tree_helpers_answer_children_and_descendants() {
        let root = AgentId::new();
        let child = AgentId::new();
        let grandchild = AgentId::new();

        let mut c = sample(None);
        c.id = child.clone();
        c.ancestors = vec![root.clone()];
        let mut g = sample(None);
        g.id = grandchild.clone();
        g.ancestors = vec![root.clone(), child.clone()];
        let outsider = sample(None);

        assert_eq!(c.distance_from(&root), Some(1));
        assert_eq!(g.distance_from(&root), Some(2));
        assert_eq!(g.distance_from(&child), Some(1));
        assert_eq!(outsider.distance_from(&root), None);
        assert!(g.is_descendant_of(&root));
        assert!(!outsider.is_descendant_of(&root));
        // The chain is what makes a grandchild reachable even if the agent in
        // between has exited: nothing here consults the middle entry.
        assert_eq!(c.parent().map(AgentId::as_str), Some(root.as_str()));
        assert_eq!(g.parent().map(AgentId::as_str), Some(child.as_str()));
    }

    #[test]
    fn the_ancestor_chain_survives_the_environment_encoding() {
        let a = AgentId::new();
        let b = AgentId::new();
        let encoded = ancestors_to_env(&[a.clone(), b.clone()]);
        assert_eq!(encoded, format!("{},{}", a.as_str(), b.as_str()));
        // Decoding happens through the environment, so exercise the parser the
        // same way `ancestors_from_env` does.
        let decoded: Vec<AgentId> = encoded
            .split(',')
            .filter_map(|s| s.trim().parse::<AgentId>().ok())
            .collect();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[1].as_str(), b.as_str());
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
