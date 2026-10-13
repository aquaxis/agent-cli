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

const BUILTIN_BODY: &str = "You are a general-purpose AI assistant running on agent-cli.\n\
Respond to user requests concisely and accurately, using tools as needed.";

impl Persona {
    pub fn builtin_default() -> Self {
        Persona {
            frontmatter: PersonaFrontmatter {
                name: Some("default".into()),
                role: "General-purpose assistant".into(),
                skills: vec!["Conversation".into(), "Tool execution".into()],
                description: Some("Built-in default persona".into()),
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
        s.push_str("\n\n# Role\n");
        s.push_str(&self.frontmatter.role);
        if !self.frontmatter.skills.is_empty() {
            s.push_str("\n\n# Skills\n");
            for sk in &self.frontmatter.skills {
                s.push_str(&format!("- {sk}\n"));
            }
        }
        if let Some(desc) = &self.frontmatter.description {
            if !desc.trim().is_empty() {
                s.push_str("\n# Description\n");
                s.push_str(desc);
                s.push('\n');
            }
        }
        if !self.body.trim().is_empty() {
            s.push_str("\n# Details\n");
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
    /// The project-local `.agent-cli/persona.md` is what resolved. Reported at
    /// load so a discovered persona is never a silent one.
    pub project_used: bool,
}

/// Resolve the persona this agent runs as. First hit wins:
///
/// 1. `--persona <path>` — explicit, missing is an error
/// 2. `[runtime] persona_file` — explicit, missing is an error
/// 3. `<agents_dir>/<name>.md` — convention, missing falls through
/// 4. `.agent-cli/persona.md` — the project's own role, missing falls through
/// 5. the built-in default
///
/// `project_persona` is the candidate path of step 4 (see
/// `config::local_persona_candidate_in`); its existence is tested here, so a
/// caller may hold the same candidate across a session. The two explicit
/// mechanisms error on a missing file because the user asked for that file;
/// the two conventions do not, because absent means "not configured". A
/// convention file that *exists* and fails to load is still an error — a wrong
/// role must not be presented as an intended one.
pub fn resolve(
    cli_path: Option<&Path>,
    runtime_persona_file: &str,
    agents_dir: &Path,
    name: Option<&str>,
    project_persona: Option<&Path>,
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
            project_used: false,
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
            project_used: false,
        });
    }
    if let Some(n) = name {
        let candidate = agents_dir.join(format!("{n}.md"));
        if candidate.exists() {
            return Ok(PersonaResolution {
                persona: Persona::load(&candidate)?,
                builtin_used: false,
                project_used: false,
            });
        }
    }
    if let Some(p) = project_persona {
        if p.is_file() {
            // Nothing on the command line pointed here, so the error has to say
            // where "here" is: `Persona::load` names the file when it cannot be
            // read, but its frontmatter faults describe only the fault.
            let persona = Persona::load(p).map_err(|e| match e {
                AppError::Persona(msg) => {
                    AppError::persona(format!("project persona {}: {msg}", p.display()))
                }
                other => other,
            })?;
            return Ok(PersonaResolution {
                persona,
                builtin_used: false,
                project_used: true,
            });
        }
    }
    Ok(PersonaResolution {
        persona: Persona::builtin_default(),
        builtin_used: true,
        project_used: false,
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
            "---\nname: alice\nrole: Code reviewer\nskills:\n  - Rust\n  - Static analysis\n---\nYou are a senior reviewer.",
        )
        .unwrap();
        let p = Persona::load(&path).unwrap();
        assert_eq!(p.frontmatter.role, "Code reviewer");
        assert_eq!(p.frontmatter.skills.len(), 2);
        assert!(p.body.contains("senior"));
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
        let r = resolve(None, "", dir.path(), Some("none"), None).unwrap();
        assert!(r.builtin_used);
        assert!(!r.project_used);
        assert_eq!(r.persona.frontmatter.role, "General-purpose assistant");
    }

    /// A project persona, written as `.agent-cli/persona.md` under `dir`.
    fn write_project_persona(dir: &Path, role: &str) -> PathBuf {
        let path = crate::config::local_persona_candidate_in(dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            format!("---\nname: project\nrole: {role}\n---\nProject instructions."),
        )
        .unwrap();
        path
    }

    /// A persona file at `<dir>/<name>.md`, the `<agents_dir>/<name>.md`
    /// convention.
    fn write_named_persona(dir: &Path, name: &str, role: &str) -> PathBuf {
        let path = dir.join(format!("{name}.md"));
        std::fs::write(&path, format!("---\nname: {name}\nrole: {role}\n---\nbody")).unwrap();
        path
    }

    /// V-3: with the three earlier steps missing, `.agent-cli/persona.md` is
    /// the role the agent runs as — the directory states it.
    #[test]
    fn project_persona_is_used_when_nothing_else_matches() {
        let project = TempDir::new().unwrap();
        let agents = TempDir::new().unwrap();
        let path = write_project_persona(project.path(), "Project reviewer");

        let r = resolve(None, "", agents.path(), Some("unknown"), Some(&path)).unwrap();

        assert_eq!(r.persona.frontmatter.role, "Project reviewer");
        assert_eq!(r.persona.source_path.as_deref(), Some(path.as_path()));
        assert!(r.project_used);
        assert!(!r.builtin_used);
        // The body reaches the system prompt like any other persona's.
        assert!(r
            .persona
            .to_system_prompt()
            .contains("Project instructions."));
    }

    /// V-4: every mechanism that resolves a persona today keeps winning — the
    /// project file is additive, not a new top priority.
    #[test]
    fn explicit_and_named_personas_outrank_the_project_persona() {
        let project = TempDir::new().unwrap();
        let agents = TempDir::new().unwrap();
        let local = write_project_persona(project.path(), "Project reviewer");
        let explicit = write_named_persona(project.path(), "explicit", "Explicit role");
        let named = write_named_persona(agents.path(), "alice", "Named role");

        // 1. `--persona` wins.
        let r = resolve(
            Some(&explicit),
            "",
            agents.path(),
            Some("alice"),
            Some(&local),
        )
        .unwrap();
        assert_eq!(r.persona.frontmatter.role, "Explicit role");
        assert!(!r.project_used);

        // 2. `[runtime] persona_file` wins.
        let r = resolve(
            None,
            explicit.to_str().unwrap(),
            agents.path(),
            Some("alice"),
            Some(&local),
        )
        .unwrap();
        assert_eq!(r.persona.frontmatter.role, "Explicit role");
        assert!(!r.project_used);

        // 3. `<agents_dir>/<name>.md` wins — a spawned `--name alice` keeps its
        //    own role in a directory that happens to carry a project persona.
        let r = resolve(None, "", agents.path(), Some("alice"), Some(&local)).unwrap();
        assert_eq!(r.persona.frontmatter.role, "Named role");
        assert_eq!(r.persona.source_path.as_deref(), Some(named.as_path()));
        assert!(!r.project_used);
    }

    /// V-5: absent is not an error — a missing `persona.md`, and a bare
    /// `.agent-cli` directory, both fall through to the built-in default.
    #[test]
    fn absent_project_persona_falls_through_to_builtin() {
        let project = TempDir::new().unwrap();
        let agents = TempDir::new().unwrap();
        let candidate = crate::config::local_persona_candidate_in(project.path());

        // Nothing there at all.
        let r = resolve(None, "", agents.path(), None, Some(&candidate)).unwrap();
        assert!(r.builtin_used);
        assert!(!r.project_used);

        // A bare `.agent-cli` directory is not a match either.
        std::fs::create_dir_all(candidate.parent().unwrap()).unwrap();
        let r = resolve(None, "", agents.path(), None, Some(&candidate)).unwrap();
        assert!(r.builtin_used);
        assert!(!r.project_used);

        // Neither is a *directory* named persona.md.
        std::fs::create_dir(&candidate).unwrap();
        let r = resolve(None, "", agents.path(), None, Some(&candidate)).unwrap();
        assert!(r.builtin_used);
    }

    /// V-6: present but malformed is an error naming the file — a wrong role
    /// must not be presented as an intended one.
    #[test]
    fn malformed_project_persona_errors_instead_of_falling_back() {
        let agents = TempDir::new().unwrap();
        for bad in [
            "no frontmatter at all",
            "---\nname: x\nrole: \n---\nbody",
            "---\nrole: [unclosed\n---\nbody",
            "---\nname: x\n---\nbody",
        ] {
            let project = TempDir::new().unwrap();
            let path = crate::config::local_persona_candidate_in(project.path());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bad).unwrap();

            let err = resolve(None, "", agents.path(), None, Some(&path)).unwrap_err();
            assert!(
                matches!(err, AppError::Persona(_)),
                "expected a persona error for {bad:?}, got {err:?}"
            );
            // The file has to be named: nothing on the command line pointed
            // here, so the message is the only way to find it.
            let msg = err.to_string();
            assert!(
                msg.contains(&path.display().to_string()),
                "error for {bad:?} does not name the file: {msg}"
            );
        }
    }

    /// V-3 / D-4: the candidate is a path, not a snapshot of existence — a
    /// `persona.md` written after the candidate was resolved is picked up by
    /// the next resolution, which is what makes `/reload-persona` work.
    #[test]
    fn a_persona_written_after_startup_is_picked_up_on_the_next_resolution() {
        let project = TempDir::new().unwrap();
        let agents = TempDir::new().unwrap();
        let candidate = crate::config::local_persona_candidate_in(project.path());

        let r = resolve(None, "", agents.path(), None, Some(&candidate)).unwrap();
        assert!(r.builtin_used);

        write_project_persona(project.path(), "Arrived later");

        let r = resolve(None, "", agents.path(), None, Some(&candidate)).unwrap();
        assert!(r.project_used);
        assert_eq!(r.persona.frontmatter.role, "Arrived later");
    }

    /// Document consistency check (T-602-10):
    /// All bundled sample personas must be loadable via `Persona::load`.
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
