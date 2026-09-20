//! Per-call permission rules for tools the model asks to run.
//!
//! Two gates stand between the model and a tool. The first decides **which
//! tools exist** — `[tools] enabled` intersected with a persona's
//! `allowed_tools` and minus its `denied_tools` — and works at whole-tool
//! granularity, once, at startup. This module is the second and finer one: for
//! a tool that does exist, may it run with the arguments the model chose?
//!
//! Everything here is a function of its inputs. Rules are parsed and compiled
//! once; deciding a call is a bounded walk of the compiled list. No file is
//! read, no terminal is touched, and nothing is invoked — the caller in
//! `agent.rs` does that, or does not.
//!
//! # What this is not
//!
//! A guardrail against mistakes, not a sandbox. Only the leading words of a
//! command are inspected, so `bash(rm:*)` does not stop `/bin/rm`, `sh -c rm`,
//! a shell alias, a script that calls it, or `cd x && rm -rf /`. Anyone who
//! needs a real boundary needs a container.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use serde_json::Value;

use crate::config::PermissionsConfig;

/// What a rule matches against, once the `tool(...)` wrapper is off.
#[derive(Debug, Clone)]
pub enum Pattern {
    /// `bash` — every call of the tool, whatever its arguments.
    Any,
    /// `bash(git:*)` — the command's leading words. Claude Code's form.
    Prefix(Vec<String>),
    /// `webfetch(domain:github.com)` — the URL's host.
    Domain(GlobMatcher),
    /// `read(.env*)` — a glob over the tool's gated argument.
    Glob(GlobMatcher),
}

/// One parsed rule, with enough provenance to say where it came from when it
/// fires. A refusal that cannot name the rule that caused it is a refusal the
/// user cannot act on.
#[derive(Debug, Clone)]
pub struct Rule {
    /// agent-cli's canonical tool name, whatever spelling the rule used.
    pub tool: String,
    pub pattern: Pattern,
    /// The rule exactly as written, so messages quote the user rather than a
    /// reconstruction of what they meant.
    pub raw: String,
}

/// The outcome for one tool call.
#[derive(Debug, Clone)]
pub enum Decision {
    /// Refused. Not invoked, and `auto_approve` is not consulted.
    Deny(Rule),
    /// Run without asking.
    Allow(Rule),
    /// Hand to the existing interactive approval flow.
    Ask,
}

/// What happens to a call no rule matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultMode {
    Ask,
    Allow,
    Deny,
}

impl DefaultMode {
    fn as_str(self) -> &'static str {
        match self {
            DefaultMode::Ask => "ask",
            DefaultMode::Allow => "allow",
            DefaultMode::Deny => "deny",
        }
    }
}

impl std::fmt::Display for DefaultMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The compiled rules, ready to decide calls.
#[derive(Debug, Clone, Default)]
pub struct Ruleset {
    deny: Vec<Rule>,
    allow: Vec<Rule>,
    default_mode: Option<DefaultMode>,
    /// Everything the configuration said that could not be honoured. Never
    /// discarded: a permission rule that silently does nothing is the failure
    /// this whole feature exists to end.
    warnings: Vec<String>,
}

impl Ruleset {
    /// Compile a `[permissions]` section. Never fails: a rule that cannot be
    /// understood becomes a warning, because a configuration that refused to
    /// start would be a worse failure than an inert rule — and the warning is
    /// what stops it being silent.
    pub fn from_config(cfg: &PermissionsConfig) -> Self {
        let mut set = Ruleset::default();
        set.deny = set.compile(&cfg.deny, "deny");
        set.allow = set.compile_into(&cfg.allow, "allow");
        set.default_mode = match cfg.default_mode.as_deref() {
            None => None,
            Some(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                "ask" => Some(DefaultMode::Ask),
                "allow" => Some(DefaultMode::Allow),
                "deny" => Some(DefaultMode::Deny),
                other => {
                    set.warnings.push(format!(
                        "default_mode = \"{other}\" is not a mode agent-cli has \
                         (use \"ask\", \"allow\" or \"deny\"); treating it as \"ask\""
                    ));
                    Some(DefaultMode::Ask)
                }
            },
        };
        set
    }

    fn compile(&mut self, raws: &[String], list: &str) -> Vec<Rule> {
        self.compile_into(raws, list)
    }

    fn compile_into(&mut self, raws: &[String], list: &str) -> Vec<Rule> {
        let mut out = Vec::with_capacity(raws.len());
        for raw in raws {
            match parse_rule(raw) {
                Ok(rule) => {
                    if let Some(note) = inert_reason(&rule) {
                        self.warnings
                            .push(format!("{list} rule {raw:?} {note}"));
                    }
                    out.push(rule);
                }
                Err(why) => self.warnings.push(format!("{list} rule {raw:?}: {why}")),
            }
        }
        out
    }

    pub fn deny_len(&self) -> usize {
        self.deny.len()
    }

    pub fn allow_len(&self) -> usize {
        self.allow.len()
    }

    pub fn default_mode(&self) -> DefaultMode {
        self.default_mode.unwrap_or(DefaultMode::Ask)
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// True when nothing was configured at all, in which case the caller keeps
    /// the behaviour it had before this module existed.
    pub fn is_empty(&self) -> bool {
        self.deny.is_empty() && self.allow.is_empty() && self.default_mode.is_none()
    }

    pub fn deny_rules(&self) -> &[Rule] {
        &self.deny
    }

    pub fn allow_rules(&self) -> &[Rule] {
        &self.allow
    }

    /// Decide one call.
    ///
    /// `deny` is scanned first and in full, then `allow`, then the default
    /// mode. The order is the point, not an optimisation: a `deny` has to
    /// outrank a matching `allow`, or a deny list means nothing.
    pub fn decide(&self, tool: &str, args: &Value) -> Decision {
        let canonical = canonical_tool(tool);
        let subject = subject_of(&canonical, args);
        for rule in &self.deny {
            if rule.matches(&canonical, subject.as_deref()) {
                return Decision::Deny(rule.clone());
            }
        }
        for rule in &self.allow {
            if rule.matches(&canonical, subject.as_deref()) {
                return Decision::Allow(rule.clone());
            }
        }
        match self.default_mode() {
            DefaultMode::Ask => Decision::Ask,
            DefaultMode::Allow => Decision::Allow(Rule {
                tool: canonical,
                pattern: Pattern::Any,
                raw: "default_mode = \"allow\"".to_string(),
            }),
            DefaultMode::Deny => Decision::Deny(Rule {
                tool: canonical,
                pattern: Pattern::Any,
                raw: "default_mode = \"deny\"".to_string(),
            }),
        }
    }
}

impl Rule {
    fn matches(&self, tool: &str, subject: Option<&str>) -> bool {
        if self.tool != tool {
            return false;
        }
        match &self.pattern {
            Pattern::Any => true,
            Pattern::Prefix(words) => match subject {
                Some(s) => command_starts_with(s, words),
                None => false,
            },
            Pattern::Domain(m) => match subject {
                // The subject for a URL tool is already reduced to its host.
                Some(host) => m.is_match(host),
                None => false,
            },
            Pattern::Glob(m) => match subject {
                Some(s) => matches_path_or_text(m, s),
                None => false,
            },
        }
    }
}

/// Parse `tool` or `tool(pattern)`.
fn parse_rule(raw: &str) -> std::result::Result<Rule, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("is empty".to_string());
    }
    let (name, inner) = match trimmed.find('(') {
        Some(open) => {
            if !trimmed.ends_with(')') {
                return Err("has an unclosed '('".to_string());
            }
            (&trimmed[..open], Some(&trimmed[open + 1..trimmed.len() - 1]))
        }
        None => {
            if trimmed.ends_with(')') {
                return Err("has a ')' with no '('".to_string());
            }
            (trimmed, None)
        }
    };
    let tool = canonical_tool(name);
    if tool.is_empty() {
        return Err("names no tool".to_string());
    }
    let pattern = match inner {
        None => Pattern::Any,
        Some(p) => {
            let p = p.trim();
            if p.is_empty() {
                return Err("has an empty pattern; write the bare tool name to match every call"
                    .to_string());
            }
            compile_pattern(p)?
        }
    };
    Ok(Rule {
        tool,
        pattern,
        raw: trimmed.to_string(),
    })
}

fn compile_pattern(p: &str) -> std::result::Result<Pattern, String> {
    if let Some(host) = p.strip_prefix("domain:") {
        let host = host.trim();
        if host.is_empty() {
            return Err("has an empty domain".to_string());
        }
        return Ok(Pattern::Domain(build_glob(host)?));
    }
    // Claude Code's `Bash(git:*)`: the leading words of the command, with the
    // `*` standing for "and whatever follows".
    if let Some(colon) = p.find(':') {
        let head = p[..colon].trim();
        if !head.is_empty() {
            let words: Vec<String> = head.split_whitespace().map(str::to_string).collect();
            if !words.is_empty() {
                return Ok(Pattern::Prefix(words));
            }
        }
    }
    Ok(Pattern::Glob(build_glob(p)?))
}

fn build_glob(p: &str) -> std::result::Result<GlobMatcher, String> {
    Glob::new(p)
        .map(|g| g.compile_matcher())
        .map_err(|e| format!("is not a valid pattern: {e}"))
}

/// A tool name as this codebase spells it.
///
/// Comparison is case-insensitive and ignores `_`, so `Bash`, `bash`,
/// `WebFetch`, `webfetch`, `SendTo` and `send_to` all land on the same tool and
/// a rule transcribed from a Claude Code settings file works unedited. The
/// canonical spelling is agent-cli's own; it is what documentation and
/// `/permissions` print.
pub fn canonical_tool(name: &str) -> String {
    let key: String = name
        .trim()
        .chars()
        .filter(|c| *c != '_')
        .flat_map(char::to_lowercase)
        .collect();
    let canonical = match key.as_str() {
        "bash" | "shell" => "bash",
        "read" | "fsread" => "read",
        "write" | "fswrite" => "write",
        "edit" => "edit",
        "glob" => "glob",
        "grep" => "grep",
        "monitor" => "monitor",
        "websearch" => "websearch",
        "webfetch" => "webfetch",
        "sendto" => "send_to",
        "spawn" => "spawn",
        "listagents" => "list_agents",
        "stopagent" => "stop_agent",
        // Anything else — an MCP tool, or a name agent-cli does not have — is
        // kept as the user wrote it, lowercased. An unknown name is reported as
        // inert rather than rejected.
        _ => return key,
    };
    canonical.to_string()
}

/// The one argument a rule for `tool` is matched against, or `None` for a tool
/// that has no argument worth gating.
///
/// Fixed and exhaustive on purpose: a tool whose subject is a guess is a tool
/// whose rules cannot be trusted. A new tool adds a line here or records that
/// it has none — see `CONTRIBUTING.md`.
pub fn subject_of(tool: &str, args: &Value) -> Option<String> {
    let field = match tool {
        "bash" | "monitor" => "command",
        "read" | "write" | "edit" => "file_path",
        // `pattern` is a *search* pattern, not a path, and must never be the
        // subject: `grep(secret)` would otherwise deny searching for a word.
        "glob" | "grep" => {
            return Some(
                args.get("path")
                    .and_then(Value::as_str)
                    .unwrap_or(".")
                    .to_string(),
            )
        }
        "webfetch" => {
            let url = args.get("url").and_then(Value::as_str)?;
            return Some(host_of(url));
        }
        "send_to" | "stop_agent" => "peer",
        _ => return None,
    };
    args.get(field).and_then(Value::as_str).map(str::to_string)
}

/// True when the tool takes no gated argument, so only a bare `tool` rule can
/// ever apply to it.
fn is_subjectless(tool: &str) -> bool {
    !matches!(
        tool,
        "bash"
            | "monitor"
            | "read"
            | "write"
            | "edit"
            | "glob"
            | "grep"
            | "webfetch"
            | "send_to"
            | "stop_agent"
    )
}

/// Why a syntactically valid rule will never fire, if it never will.
fn inert_reason(rule: &Rule) -> Option<&'static str> {
    if matches!(rule.pattern, Pattern::Any) {
        return None;
    }
    if is_subjectless(&rule.tool) {
        return Some(
            "writes a pattern for a tool that has no gated argument; \
             it will never match — use the bare tool name to cover every call",
        );
    }
    None
}

/// The host of a URL, lowercased, or the whole string when it does not parse as
/// one — so a malformed URL is still matchable rather than silently unmatched.
fn host_of(url: &str) -> String {
    let rest = url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(url);
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = match authority.rfind(':') {
        // Not a port when the colon belongs to an IPv6 literal.
        Some(i) if !authority.contains(']') => &authority[..i],
        _ => authority,
    };
    host.to_ascii_lowercase()
}

/// Whether `command` begins with `words`, word by word.
///
/// Word-by-word rather than a string prefix, so `bash(git:*)` matches
/// `git status` but not `github-cli whatever`.
fn command_starts_with(command: &str, words: &[String]) -> bool {
    let mut actual = command.split_whitespace();
    for expected in words {
        match actual.next() {
            Some(got) if got == expected => {}
            _ => return false,
        }
    }
    true
}

/// Match a glob against a subject, and — when the subject is a path — against
/// its resolved absolute form too.
///
/// Without the second form, `read(.env*)` is defeated by writing `./.env`, and
/// a deny defeated by two characters is not a deny. Resolution is lexical
/// rather than `canonicalize`, which fails on a path that does not exist —
/// `write` and `edit` create files.
fn matches_path_or_text(m: &GlobMatcher, subject: &str) -> bool {
    if m.is_match(subject) {
        return true;
    }
    for form in path_forms(subject) {
        if m.is_match(&form) {
            return true;
        }
    }
    false
}

/// The alternative spellings of `subject` as a path: `~` expanded, `./` peeled
/// off, and the absolute form. Empty when the subject is not path-like enough
/// for any of them to differ.
fn path_forms(subject: &str) -> Vec<String> {
    let mut out = Vec::new();
    let expanded = shellexpand::tilde(subject).into_owned();
    if expanded != subject {
        out.push(expanded.clone());
    }
    let trimmed = expanded.trim_start_matches("./").to_string();
    if trimmed != subject && !trimmed.is_empty() {
        out.push(trimmed.clone());
    }
    let absolute: PathBuf = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        match std::env::current_dir() {
            Ok(cwd) => lexical_join(&cwd, &trimmed),
            Err(_) => return out,
        }
    };
    let absolute = absolute.to_string_lossy().into_owned();
    if absolute != subject && !out.contains(&absolute) {
        out.push(absolute);
    }
    out
}

/// Join `rel` onto `base` and resolve `.`/`..` textually. `canonicalize` cannot
/// be used: the file may not exist yet.
fn lexical_join(base: &Path, rel: &str) -> PathBuf {
    let mut parts: Vec<std::ffi::OsString> = base
        .components()
        .map(|c| c.as_os_str().to_os_string())
        .collect();
    for part in Path::new(rel).components() {
        use std::path::Component;
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.len() > 1 {
                    parts.pop();
                }
            }
            other => parts.push(other.as_os_str().to_os_string()),
        }
    }
    parts.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(deny: &[&str], allow: &[&str], mode: Option<&str>) -> Ruleset {
        Ruleset::from_config(&PermissionsConfig {
            deny: deny.iter().map(|s| s.to_string()).collect(),
            allow: allow.iter().map(|s| s.to_string()).collect(),
            default_mode: mode.map(str::to_string),
        })
    }

    fn is_deny(d: &Decision) -> bool {
        matches!(d, Decision::Deny(_))
    }
    fn is_allow(d: &Decision) -> bool {
        matches!(d, Decision::Allow(_))
    }
    fn is_ask(d: &Decision) -> bool {
        matches!(d, Decision::Ask)
    }

    // --- V-16: nothing configured leaves the previous behaviour alone --------

    #[test]
    fn no_rules_at_all_asks_for_everything() {
        let s = set(&[], &[], None);
        assert!(s.is_empty(), "an unconfigured section must read as empty");
        assert!(is_ask(&s.decide("bash", &json!({"command": "rm -rf /"}))));
        assert!(is_ask(&s.decide("read", &json!({"file_path": ".env"}))));
        assert!(s.warnings().is_empty());
    }

    // --- V-9: the three pattern shapes --------------------------------------

    #[test]
    fn a_prefix_rule_matches_leading_words_and_not_a_longer_first_word() {
        let s = set(&["bash(git:*)"], &[], None);
        assert!(is_deny(&s.decide("bash", &json!({"command": "git status"}))));
        assert!(is_deny(&s.decide("bash", &json!({"command": "git push --force"}))));
        // The whole reason matching is word-by-word rather than a string prefix.
        assert!(is_ask(&s.decide("bash", &json!({"command": "github-cli x"}))));
        assert!(is_ask(&s.decide("bash", &json!({"command": "gitk"}))));
    }

    #[test]
    fn a_multi_word_prefix_rule_needs_every_word() {
        let s = set(&["bash(npm publish:*)"], &[], None);
        assert!(is_deny(&s.decide("bash", &json!({"command": "npm publish --tag x"}))));
        assert!(is_ask(&s.decide("bash", &json!({"command": "npm install"}))));
    }

    #[test]
    fn a_domain_rule_matches_the_host_only() {
        let s = set(&["webfetch(domain:github.com)"], &[], None);
        assert!(is_deny(
            &s.decide("webfetch", &json!({"url": "https://github.com/a/b?x=1"}))
        ));
        assert!(is_ask(
            &s.decide("webfetch", &json!({"url": "https://example.com/github.com"}))
        ));
    }

    #[test]
    fn a_domain_wildcard_matches_any_host() {
        let s = set(&[], &["webfetch(domain:*)"], None);
        assert!(is_allow(
            &s.decide("webfetch", &json!({"url": "https://anything.example/x"}))
        ));
    }

    #[test]
    fn a_glob_rule_matches_the_subject() {
        let s = set(&["bash(rm -rf ~/**)"], &[], None);
        assert!(is_deny(
            &s.decide("bash", &json!({"command": "rm -rf ~/Documents"}))
        ));
        assert!(is_ask(&s.decide("bash", &json!({"command": "rm -rf ./build"}))));
    }

    #[test]
    fn a_bare_tool_rule_matches_every_call() {
        let s = set(&["websearch"], &[], None);
        assert!(is_deny(&s.decide("websearch", &json!({"query": "anything"}))));
    }

    // --- V-10: tool-name spellings ------------------------------------------

    #[test]
    fn tool_names_resolve_across_the_three_spellings() {
        for spelling in ["Bash", "bash", "BASH"] {
            let s = set(&[&format!("{spelling}(git:*)")], &[], None);
            assert!(
                is_deny(&s.decide("bash", &json!({"command": "git log"}))),
                "{spelling} should name the bash tool"
            );
        }
        let s = set(&["WebFetch(domain:x.test)"], &[], None);
        assert!(is_deny(&s.decide("webfetch", &json!({"url": "http://x.test/"}))));

        let s = set(&["SendTo(worker-*)"], &[], None);
        assert!(is_deny(&s.decide("send_to", &json!({"peer": "worker-1"}))));
        assert!(is_ask(&s.decide("send_to", &json!({"peer": "boss"}))));
    }

    // --- V-11: the subject table --------------------------------------------

    #[test]
    fn every_tool_gates_on_the_argument_it_should() {
        assert_eq!(
            subject_of("bash", &json!({"command": "ls"})).as_deref(),
            Some("ls")
        );
        assert_eq!(
            subject_of("monitor", &json!({"command": "tail -f x"})).as_deref(),
            Some("tail -f x")
        );
        for tool in ["read", "write", "edit"] {
            assert_eq!(
                subject_of(tool, &json!({"file_path": "/tmp/a"})).as_deref(),
                Some("/tmp/a"),
                "{tool} gates on file_path"
            );
        }
        assert_eq!(
            subject_of("send_to", &json!({"peer": "w1"})).as_deref(),
            Some("w1")
        );
        assert_eq!(
            subject_of("webfetch", &json!({"url": "https://A.Example:8443/p"})).as_deref(),
            Some("a.example")
        );
    }

    #[test]
    fn glob_and_grep_gate_on_path_and_never_on_their_search_pattern() {
        // `grep(secret)` must not mean "may not search for the word secret".
        assert_eq!(
            subject_of("grep", &json!({"pattern": "secret", "path": "src"})).as_deref(),
            Some("src")
        );
        assert_eq!(
            subject_of("glob", &json!({"pattern": "**/*.rs"})).as_deref(),
            Some("."),
            "an absent path is the working directory"
        );
        let s = set(&["grep(secret)"], &[], None);
        assert!(is_ask(
            &s.decide("grep", &json!({"pattern": "secret", "path": "src"}))
        ));
    }

    #[test]
    fn subjectless_tools_have_no_subject() {
        for tool in ["websearch", "list_agents", "spawn", "mcp__srv__thing"] {
            assert!(
                subject_of(tool, &json!({"query": "x"})).is_none(),
                "{tool} has no gated argument"
            );
        }
    }

    // --- V-12: paths are matched as written and resolved --------------------

    #[test]
    fn a_path_rule_is_not_defeated_by_a_leading_dot_slash() {
        let s = set(&["read(.env*)"], &[], None);
        assert!(is_deny(&s.decide("read", &json!({"file_path": ".env"}))));
        assert!(is_deny(&s.decide("read", &json!({"file_path": "./.env"}))));
        assert!(is_deny(
            &s.decide("read", &json!({"file_path": "./.env.local"}))
        ));
    }

    #[test]
    fn an_absolute_path_under_the_working_directory_still_matches() {
        let cwd = std::env::current_dir().unwrap();
        let absolute = cwd.join(".env").to_string_lossy().into_owned();
        let s = set(&["read(**/.env*)"], &[], None);
        assert!(is_deny(&s.decide("read", &json!({"file_path": absolute}))));
    }

    #[test]
    fn a_path_that_does_not_exist_yet_still_matches() {
        // `write` and `edit` create files, so resolution must not need the file.
        let s = set(&["write(**/secrets/**)"], &[], None);
        assert!(is_deny(
            &s.decide("write", &json!({"file_path": "./secrets/new/file.txt"}))
        ));
    }

    // --- V-13: precedence ---------------------------------------------------

    #[test]
    fn deny_beats_an_allow_that_also_matches() {
        // Both rules match the same call: the ordering is what is under test.
        let s = set(&["bash(git push:*)"], &["bash(git:*)"], None);
        assert!(is_deny(
            &s.decide("bash", &json!({"command": "git push --force"}))
        ));
        assert!(is_allow(&s.decide("bash", &json!({"command": "git status"}))));
    }

    #[test]
    fn deny_beats_a_permissive_default_mode() {
        let s = set(&["bash(rm:*)"], &[], Some("allow"));
        assert!(is_deny(&s.decide("bash", &json!({"command": "rm -rf /"}))));
        assert!(is_allow(&s.decide("bash", &json!({"command": "ls"}))));
    }

    #[test]
    fn a_restrictive_default_mode_denies_what_no_rule_allowed() {
        let s = set(&[], &["bash(git:*)"], Some("deny"));
        assert!(is_allow(&s.decide("bash", &json!({"command": "git log"}))));
        assert!(is_deny(&s.decide("bash", &json!({"command": "curl x"}))));
    }

    // --- V-14: default_mode -------------------------------------------------

    #[test]
    fn each_default_mode_is_understood_and_an_absent_one_asks() {
        assert_eq!(set(&[], &[], None).default_mode(), DefaultMode::Ask);
        assert_eq!(set(&[], &[], Some("ask")).default_mode(), DefaultMode::Ask);
        assert_eq!(set(&[], &[], Some("allow")).default_mode(), DefaultMode::Allow);
        assert_eq!(set(&[], &[], Some("deny")).default_mode(), DefaultMode::Deny);
    }

    #[test]
    fn a_mode_agent_cli_does_not_have_warns_and_falls_back_to_ask() {
        // Claude Code's file says "acceptEdits"; agent-cli has no edit-versus-
        // everything-else distinction to hang it on.
        let s = set(&[], &[], Some("acceptEdits"));
        assert_eq!(s.default_mode(), DefaultMode::Ask);
        assert_eq!(s.warnings().len(), 1);
        assert!(s.warnings()[0].contains("acceptedits") || s.warnings()[0].contains("acceptEdits"));
    }

    // --- V-15: every warning path -------------------------------------------

    #[test]
    fn a_rule_naming_an_unknown_tool_is_inert_and_not_fatal() {
        // `MultiEdit` is a Claude Code tool with no agent-cli equivalent.
        let s = set(&["MultiEdit(**)"], &[], None);
        assert!(is_ask(&s.decide("edit", &json!({"file_path": "a"}))));
        assert_eq!(s.deny_len(), 1, "the rule is kept, just never matched");
    }

    #[test]
    fn a_pattern_on_a_subjectless_tool_warns_instead_of_silently_never_matching() {
        let s = set(&["websearch(rust)"], &[], None);
        assert_eq!(s.warnings().len(), 1, "{:?}", s.warnings());
        assert!(s.warnings()[0].contains("websearch(rust)"));
        assert!(s.warnings()[0].contains("never match"));
    }

    #[test]
    fn an_unparseable_rule_warns_and_quotes_itself() {
        let s = set(&["bash(git:*", "read(["], &[], None);
        assert_eq!(s.warnings().len(), 2, "{:?}", s.warnings());
        assert!(s.warnings()[0].contains("bash(git:*"));
        assert!(s.warnings().iter().any(|w| w.contains("unclosed")));
        assert_eq!(s.deny_len(), 0, "neither rule was compiled");
    }

    #[test]
    fn an_empty_pattern_is_rejected_rather_than_matching_everything() {
        // `bash()` reading as "deny every bash call" would be a booby trap.
        let s = set(&["bash()"], &[], None);
        assert_eq!(s.deny_len(), 0);
        assert_eq!(s.warnings().len(), 1);
    }

    // --- V-11 / AC-06: the file this cycle deletes, transcribed --------------

    #[test]
    fn the_deleted_claude_code_rule_set_loads_with_everything_reported() {
        // The rules `.agent-cli/settings.json` carried, in its own spellings.
        let s = set(
            &[
                "Bash(rm -rf ~/**)",
                "Bash(git remote add:*)",
                "Bash(npm publish:*)",
                "Read(.env*)",
                "Read(tmp/**/*)",
            ],
            &["Bash(git:*)", "Read(**)", "MultiEdit(**)", "WebFetch(domain:*)"],
            Some("acceptEdits"),
        );
        // The rules that translate, do.
        assert!(is_deny(
            &s.decide("bash", &json!({"command": "npm publish"}))
        ));
        assert!(is_deny(&s.decide("read", &json!({"file_path": "./.env"}))));
        assert!(is_allow(&s.decide("bash", &json!({"command": "git status"}))));
        // The one that does not, is reported rather than dropped.
        assert_eq!(s.default_mode(), DefaultMode::Ask);
        assert!(
            s.warnings().iter().any(|w| w.contains("default_mode")),
            "{:?}",
            s.warnings()
        );
    }

    #[test]
    fn default_mode_accepts_claude_codes_spelling_as_an_alias() {
        let cfg: PermissionsConfig =
            toml::from_str("defaultMode = \"deny\"").expect("alias parses");
        assert_eq!(
            Ruleset::from_config(&cfg).default_mode(),
            DefaultMode::Deny
        );
    }

    // --- host parsing --------------------------------------------------------

    #[test]
    fn a_url_reduces_to_its_host() {
        assert_eq!(host_of("https://Example.COM/a/b"), "example.com");
        assert_eq!(host_of("http://user:pw@host.test:8080/x"), "host.test");
        assert_eq!(host_of("host.test/x"), "host.test");
        assert_eq!(host_of("not a url"), "not a url");
    }
}
