use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaFrontmatter {
    #[serde(default)]
    pub name: Option<String>,
    pub role: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    pub denied_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Persona {
    pub frontmatter: PersonaFrontmatter,
    pub body: String,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSummary {
    pub role: String,
    pub skills: Vec<String>,
    pub description: Option<String>,
    pub source_path: Option<PathBuf>,
}

const BUILTIN_BODY: &str = "あなたは agent-cli 上で動作する汎用 AI アシスタントです。\n\
ユーザーからの依頼に対し、必要に応じてツールを使い、簡潔かつ正確に応答してください。";

impl Persona {
    pub fn builtin_default() -> Self {
        Persona {
            frontmatter: PersonaFrontmatter {
                name: Some("default".into()),
                role: "汎用アシスタント".into(),
                skills: vec!["対話".into(), "ツール実行".into()],
                description: Some("組み込みの既定ペルソナ".into()),
                model: None,
                temperature: None,
                allowed_tools: None,
                denied_tools: None,
            },
            body: BUILTIN_BODY.to_string(),
            source_path: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            AppError::persona(format!(
                "failed to read persona file {}: {e}",
                path.display()
            ))
        })?;
        let (front, body) = split_frontmatter(&raw)?;
        let frontmatter: PersonaFrontmatter = serde_yaml::from_str(front)
            .map_err(|e| AppError::persona(format!("invalid YAML frontmatter: {e}")))?;
        if frontmatter.role.trim().is_empty() {
            return Err(AppError::persona(
                "`role` is required in persona frontmatter",
            ));
        }
        Ok(Persona {
            frontmatter,
            body: body.trim().to_string(),
            source_path: Some(path.to_path_buf()),
        })
    }

    pub fn to_system_prompt(&self) -> String {
        let mut s = String::new();
        s.push_str(BUILTIN_BODY);
        s.push_str("\n\n# 役割\n");
        s.push_str(&self.frontmatter.role);
        if !self.frontmatter.skills.is_empty() {
            s.push_str("\n\n# スキル\n");
            for sk in &self.frontmatter.skills {
                s.push_str(&format!("- {sk}\n"));
            }
        }
        if let Some(desc) = &self.frontmatter.description {
            if !desc.trim().is_empty() {
                s.push_str("\n# 説明\n");
                s.push_str(desc);
                s.push('\n');
            }
        }
        if !self.body.trim().is_empty() {
            s.push_str("\n# 詳細\n");
            s.push_str(&self.body);
        }
        s
    }

    pub fn summary(&self) -> PersonaSummary {
        PersonaSummary {
            role: self.frontmatter.role.clone(),
            skills: self.frontmatter.skills.clone(),
            description: self.frontmatter.description.clone(),
            source_path: self.source_path.clone(),
        }
    }
}

fn split_frontmatter(raw: &str) -> Result<(&str, &str)> {
    let raw = raw.trim_start_matches('\u{feff}');
    let stripped = raw.strip_prefix("---").ok_or_else(|| {
        AppError::persona("persona file must begin with YAML frontmatter (`---`)")
    })?;
    // Find next "---" delimiter
    let after_first = stripped.trim_start_matches('\n');
    let end = after_first
        .find("\n---")
        .ok_or_else(|| AppError::persona("missing closing `---` for YAML frontmatter"))?;
    let yaml = &after_first[..end];
    let rest = &after_first[end + 4..];
    let body = rest.trim_start_matches('\n');
    Ok((yaml, body))
}

#[derive(Debug, Clone)]
pub struct PersonaResolution {
    pub persona: Persona,
    pub builtin_used: bool,
}

pub fn resolve(
    cli_path: Option<&Path>,
    runtime_persona_file: &str,
    agents_dir: &Path,
    name: Option<&str>,
) -> Result<PersonaResolution> {
    if let Some(p) = cli_path {
        if !p.exists() {
            return Err(AppError::persona(format!(
                "persona file not found: {}",
                p.display()
            )));
        }
        return Ok(PersonaResolution {
            persona: Persona::load(p)?,
            builtin_used: false,
        });
    }
    if !runtime_persona_file.is_empty() {
        let p = crate::config::expand_path(runtime_persona_file)?;
        if !p.exists() {
            return Err(AppError::persona(format!(
                "persona file not found: {}",
                p.display()
            )));
        }
        return Ok(PersonaResolution {
            persona: Persona::load(&p)?,
            builtin_used: false,
        });
    }
    if let Some(n) = name {
        let candidate = agents_dir.join(format!("{n}.md"));
        if candidate.exists() {
            return Ok(PersonaResolution {
                persona: Persona::load(&candidate)?,
                builtin_used: false,
            });
        }
    }
    Ok(PersonaResolution {
        persona: Persona::builtin_default(),
        builtin_used: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_persona_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("alice.md");
        std::fs::write(
            &path,
            "---\nname: alice\nrole: コードレビュアー\nskills:\n  - Rust\n  - 静的解析\n---\nあなたは熟練のレビュアーです。",
        )
        .unwrap();
        let p = Persona::load(&path).unwrap();
        assert_eq!(p.frontmatter.role, "コードレビュアー");
        assert_eq!(p.frontmatter.skills.len(), 2);
        assert!(p.body.contains("熟練"));
    }

    #[test]
    fn missing_role_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.md");
        std::fs::write(&path, "---\nname: bad\nrole: \n---\nbody").unwrap();
        let err = Persona::load(&path).unwrap_err();
        assert!(matches!(err, AppError::Persona(_)));
    }

    #[test]
    fn builtin_used_when_nothing_specified() {
        let dir = TempDir::new().unwrap();
        let r = resolve(None, "", dir.path(), Some("none")).unwrap();
        assert!(r.builtin_used);
        assert_eq!(r.persona.frontmatter.role, "汎用アシスタント");
    }

    /// ドキュメント整合性チェック（T-602-10）：
    /// 同梱しているサンプルペルソナがすべて `Persona::load` で読み込めること。
    #[test]
    fn bundled_example_personas_parse() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = manifest_dir.join("example").join("agents");
        let entries = std::fs::read_dir(&dir).expect("example/agents missing");
        let mut count = 0;
        for entry in entries {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let p = Persona::load(&entry.path())
                .unwrap_or_else(|e| panic!("failed to load {}: {e}", entry.path().display()));
            assert!(
                !p.frontmatter.role.trim().is_empty(),
                "{} has empty role",
                entry.path().display()
            );
            count += 1;
        }
        assert!(
            count >= 3,
            "expected at least 3 sample personas, found {count}"
        );
    }
}
