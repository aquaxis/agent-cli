use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};
use crate::theme::ColorMode;

const DEFAULT_CONFIG: &str = r#"# agent-cli configuration

[provider]
# Backend to use: "claude" | "claude-code" | "codex" | "ollama" | "opencode" |
# "opencode-go" | "llama.cpp"
kind = "claude"

[provider.claude]
model       = "claude-opus-4-7"
api_key_env = "ANTHROPIC_API_KEY"
base_url    = "https://api.anthropic.com"
thinking    = true
# Opt-in: Anthropic prompt caching (system + tools + conversation tail).
# prompt_cache = true

[provider.claude-code]
# Drives the locally installed Claude Code CLI (`claude`) as a backend. No API
# key: it uses whatever authentication Claude Code itself already has.
# bin       = "claude"       # executable name (resolved via PATH) or full path
# model     = "opus"         # --model; omit to use Claude Code's own default
# mode      = "delegation"   # "delegation" (Claude Code runs its own tools)
#                            # | "gateway"  (--tools "", chat only: agent-cli's
#                            #               tools cannot be offered to it)
# transport = "stream"       # "stream" (token streaming) | "oneshot"
# session   = "persistent"   # "persistent" | "ephemeral" (delegation only)
# tools              = ["Bash", "Read"]  # --tools
# allowed_tools      = ["Bash(git *)"]   # --allowed-tools
# disallowed_tools   = ["WebFetch"]      # --disallowed-tools
# permission_mode    = "auto"            # --permission-mode
# turn_timeout_secs  = 900
# max_budget_usd     = 1.0               # --max-budget-usd
# system_prompt_mode = "append"          # "append" | "replace"
# extra_args         = []                # passed through verbatim

[provider.codex]
model       = "gpt-4.1"
api_key_env = "OPENAI_API_KEY"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider.opencode]
# --- Local mode ---
# Point base_url at a running `opencode serve` (no api_key_env needed).
model    = "claude-sonnet-4-5"
base_url = "http://127.0.0.1:4096"
# --- Cloud mode (OpenCode Zen) ---
# Set api_key_env to the environment variable holding your key.
# A resolved key selects cloud mode automatically.
# api_key_env = "OPENCODE_API_KEY"
# base_url    = "https://opencode.ai/zen/v1"
# --- Cloud mode (OpenCode Go) ---
# Shortcut: set kind = "opencode-go" above — base_url, api, model and
# api_key_env are auto-populated. Or set them manually:
# api_key_env = "OPENCODE_API_KEY"
# base_url    = "https://opencode.ai/zen/go/v1"
# api         = "openai"
# Go serves open-weight models only; list them with GET {base_url}/models.
# model       = "qwen3.8-max"
# Cloud wire format: "openai" (default, /chat/completions) or
# "anthropic" (/messages). Use the matching base_url (e.g. the "go" endpoints
# https://opencode.ai/zen/go/v1).
# api = "anthropic"
# Cloud only: the x-opencode-session id sent with every request. Generated
# per process when unset — set it only to pin one id across restarts.
# session_id = "ses_my_stable_id"
# Opt-in (local mode only): reuse one server session across turns.
# persistent_session = true

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
# Optional sampling knobs (omit any => the llama.cpp server's own default).
# Names mirror the llama-cli flags shown after each comment.
# max_tokens     = 1024   # -n / --n-predict
# temperature    = 0.2    # --temp
# top_k          = 80     # --top-k
# top_p          = 0.95   # --top-p
# min_p          = 0.05   # --min-p
# repeat_penalty = 1.05   # --repeat-penalty
# repeat_last_n  = 64     # --repeat-last-n
# seed           = 0      # --seed

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"
registry_dir       = ""
agents_dir         = "~/.config/agent-cli/agents"
persona_file       = ""
commands_dir       = ".agent-cli/commands"   # custom slash commands (*.md)
# Max tool-use iterations per turn (default 24, minimum 1).
# max_tool_iterations = 24
# Shared group id for agents this config launches; --group overrides it and
# detached children inherit it (`agent-cli list --group <id>` / `agent-cli groups`).
# group             = ""

[tools]
# `spawn` (create detached peers of your own) is the one built-in left out: it
# is opt-in because creating processes is more impactful than the rest. Add it
# here to offer it, and see [spawn] below for the limits that bound it.
enabled = ["bash", "read", "write", "send_to", "list_agents", "stop_agent", "monitor", "edit", "glob", "grep", "websearch", "webfetch"]

[tools.bash]
timeout_ms    = 120000
max_output_kb = 256

# Web search (opt-in network tool). Without endpoint / api_key_env the tool
# returns a configuration error instead of failing silently.
# [tools.websearch]
# api_key_env = "TAVILY_API_KEY"
# endpoint    = "https://api.tavily.com/search"
# provider    = "tavily"

[ui]
show_thinking    = "collapsed"
# Activity line + spinner/elapsed while a turn runs (interactive terminals only).
show_progress    = true
# Colour the output: "auto" (terminals only, honours NO_COLOR) / "always" / "never".
color            = "auto"
# Scroll the session log with the mouse wheel, keeping the prompt line pinned.
mouse_scroll     = true
# Lines of output kept for scrolling back (0 disables the wheel scrollback).
scrollback_lines = 2000
# Drag over the log to select it; releasing copies the selection as plain text.
# Shift-drag still gives the terminal its own selection.
mouse_select     = true
# Where a copied selection goes. Empty = the terminal's own clipboard via OSC 52
# (works over SSH); set a command to pipe it instead, e.g. "wl-copy".
copy_command     = ""

[spawn]
# How many live children the `spawn` *tool* may give one agent (0 disables it).
# `agent-cli spawn` and the REPL's /spawn are not bounded by these, and these
# have no effect until "spawn" is in [tools] enabled.
max_children = 4
# How deep a chain of tool-spawned agents may go.
max_depth    = 2

# [permissions]
# Per-call rules for tools the model asks to run. Whole section is optional:
# with no rules and no default_mode, every call falls through to the y/N prompt
# exactly as it did before this section existed.
#
# A rule is `tool` (every call of it) or `tool(pattern)`. Patterns come in three
# shapes: `bash(git:*)` matches a command whose leading words are `git`;
# `webfetch(domain:github.com)` matches the URL's host; anything else is a glob
# over the tool's one gated argument — `command` for bash/monitor, `file_path`
# for read/write/edit, `path` for glob/grep, `peer` for send_to/stop_agent.
#
# `deny` outranks `allow`, outranks `default_mode`, and outranks
# `auto_approve_tools` / `/auto on`. A deny that a flag can switch off is not a
# deny. These are a guardrail against mistakes, not a sandbox: `bash(rm:*)`
# does not stop `/bin/rm`, `sh -c rm`, or `cd x && rm -rf /`.
#
# deny = [
#   "bash(rm -rf ~/**)",    # unrecoverable
#   "bash(npm publish:*)",  # a mistaken publish cannot be taken back
#   "read(.env*)",          # secrets would land in the conversation log
# ]
# allow = [
#   "bash(git:*)",
#   "read(**)",
# ]
# default_mode = "ask"      # "ask" (default) | "allow" | "deny"

[shell]
# Run `!<command>` typed at the prompt (no approval: it is your own command).
enabled       = true
timeout_ms    = 120000
# How much of the output the model is given (the screen always shows all of it).
max_output_kb = 256
# Hand the command and its output to the model as context.
context       = true

[history]
# Opt-in hybrid window management. When disabled (default), the full
# conversation is replayed verbatim each turn (unchanged behavior).
enabled            = false
max_context_tokens = 24000
keep_recent_turns  = 6

# Model Context Protocol (MCP) servers. Each enabled server is launched over
# stdio at startup; its tools are registered as mcp__<name>__<tool>. Only the
# stdio transport (and MCP tools) are supported. Example:
# [mcp]
# init_timeout_ms = 15000            # per-server handshake/list timeout
#
# [[mcp.servers]]                    # stdio server (a launched subprocess)
# name    = "filesystem"
# command = "npx"
# args    = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"]
# # env     = { EXAMPLE = "1" }      # merged onto the inherited environment
# # cwd     = "/some/dir"
# # enabled = true                   # default: true
#
# [[mcp.servers]]                    # http server (Streamable HTTP)
# name        = "remote"
# transport   = "http"
# url         = "https://example.com/mcp"
# # headers     = { X-Example = "1" }         # static request headers
# # api_key_env = "REMOTE_MCP_TOKEN"          # -> Authorization: Bearer <value>
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub provider: ProviderRoot,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub shell: ShellConfig,
    #[serde(default)]
    pub spawn: SpawnConfig,
    #[serde(default)]
    pub permissions: PermissionsConfig,
}

/// `[permissions]` — what a tool may be asked to do.
///
/// This is the second of two gates and the finer one. The first decides *which
/// tools exist* (`[tools] enabled` intersected with a persona's `allowed_tools`
/// and minus its `denied_tools`) and works at whole-tool granularity. These
/// rules decide, per call, whether a tool that does exist may run with the
/// arguments the model chose.
///
/// Empty lists with no `default_mode` leave tool approval exactly as it was
/// before this section existed: everything falls through to the interactive
/// y/N prompt, which `[runtime] auto_approve_tools` may still skip.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionsConfig {
    /// Rules that refuse a call outright. A `deny` outranks an `allow`, the
    /// default mode, **and** `auto_approve_tools` — a deny list that a flag can
    /// switch off is not a deny list.
    #[serde(default)]
    pub deny: Vec<String>,
    /// Rules that run a call without asking.
    #[serde(default)]
    pub allow: Vec<String>,
    /// What happens to a call no rule matched: `"ask"` (default), `"allow"` or
    /// `"deny"`. `defaultMode` is accepted as an alias so a rule set
    /// transcribed from Claude Code loads unedited.
    #[serde(default, alias = "defaultMode")]
    pub default_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRoot {
    pub kind: String,
    #[serde(default)]
    pub claude: Option<ProviderEntry>,
    #[serde(default, rename = "claude-code")]
    pub claude_code: Option<ProviderEntry>,
    #[serde(default)]
    pub codex: Option<ProviderEntry>,
    #[serde(default)]
    pub ollama: Option<ProviderEntry>,
    #[serde(default)]
    pub opencode: Option<ProviderEntry>,
    #[serde(default, rename = "llama.cpp")]
    pub llamacpp: Option<ProviderEntry>,
    #[serde(flatten)]
    pub extras: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderEntry {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub thinking: Option<bool>,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Total HTTP request timeout in seconds, including streaming. Optional —
    /// providers fall back to a generous default (≥ 900s) so cloud reasoning
    /// models (e.g. `glm-5.1:cloud`) that emit minutes of `thinking` tokens
    /// before content do not get aborted mid-stream.
    #[serde(default)]
    pub request_timeout_secs: Option<u64>,
    /// ollama only: how many times to retry a transient provider error (HTTP 503
    /// or an "overloaded" / "please retry" body) with exponential backoff before
    /// surfacing it. Optional — falls back to a small default. `0` disables retry.
    #[serde(default)]
    pub max_retries: Option<u64>,
    /// Claude only: enable Anthropic prompt caching (`cache_control`
    /// breakpoints on system / tools / conversation tail). Opt-in;
    /// `None`/absent => disabled (behavior unchanged).
    #[serde(default)]
    pub prompt_cache: Option<bool>,
    /// opencode local mode only: reuse one OpenCode `session_id` across turns
    /// and send only new turns instead of re-flattening full history. Opt-in;
    /// `None`/absent => disabled (ephemeral session per turn, unchanged).
    #[serde(default)]
    pub persistent_session: Option<bool>,
    /// opencode **cloud** mode only: pin the `x-opencode-session` id sent with
    /// every request. `None`/absent => a stable random `ses_<ulid>` per
    /// process, which is what the gateway wants (one id per conversation).
    /// Set it only to keep one id across restarts.
    #[serde(default)]
    pub session_id: Option<String>,
    /// opencode **cloud** mode only: wire format / endpoint to use.
    /// `"openai"` (default) → OpenAI-compatible `{base}/chat/completions`;
    /// `"anthropic"` → Anthropic-compatible `{base}/messages`. Ignored in
    /// local mode and by other providers.
    #[serde(default)]
    pub api: Option<String>,
    /// llama.cpp only: max tokens to generate (`llama-cli -n / --n-predict`),
    /// forwarded as `max_tokens`. `None`/absent => server default (unchanged).
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// llama.cpp only: Top-K sampling (`llama-cli --top-k`), forwarded as
    /// `top_k`. `None`/absent => server default.
    #[serde(default)]
    pub top_k: Option<u32>,
    /// llama.cpp only: Top-P (nucleus) sampling (`llama-cli --top-p`),
    /// forwarded as `top_p`. `None`/absent => server default.
    #[serde(default)]
    pub top_p: Option<f32>,
    /// llama.cpp only: Min-P sampling (`llama-cli --min-p`), forwarded as
    /// `min_p`. `None`/absent => server default.
    #[serde(default)]
    pub min_p: Option<f32>,
    /// llama.cpp only: repetition penalty (`llama-cli --repeat-penalty`),
    /// forwarded as `repeat_penalty`. `None`/absent => server default.
    #[serde(default)]
    pub repeat_penalty: Option<f32>,
    /// llama.cpp only: window for the repeat penalty
    /// (`llama-cli --repeat-last-n`), forwarded as `repeat_last_n`.
    /// `None`/absent => server default.
    #[serde(default)]
    pub repeat_last_n: Option<i32>,
    /// llama.cpp only: RNG seed (`llama-cli --seed`), forwarded as `seed`.
    /// `None`/absent => server default (non-deterministic).
    #[serde(default)]
    pub seed: Option<u64>,
    /// claude-code only: the Claude Code executable. A value containing a path
    /// separator is used as-is; otherwise it is resolved through `PATH`.
    /// `None`/absent => `"claude"`.
    #[serde(default)]
    pub bin: Option<String>,
    /// claude-code only: who owns the agent loop. `"delegation"` (default)
    /// lets Claude Code run its own tools; `"gateway"` passes `--tools ""` and
    /// uses it as a chat-only backend. Unknown values are rejected.
    #[serde(default)]
    pub mode: Option<String>,
    /// claude-code only: `"stream"` (default, `--output-format stream-json`
    /// with token-level deltas) or `"oneshot"` (one child per turn,
    /// `--output-format json`). Unknown values are rejected.
    #[serde(default)]
    pub transport: Option<String>,
    /// claude-code only, delegation mode only: `"persistent"` (default) reuses
    /// one Claude Code session across turns and sends only new messages;
    /// `"ephemeral"` passes `--no-session-persistence` and re-sends the
    /// transcript each turn. Unknown values are rejected.
    #[serde(default)]
    pub session: Option<String>,
    /// claude-code only: built-in Claude Code tools to enable (`--tools`).
    /// `None`/absent => flag omitted (Claude Code's own default set). Ignored
    /// in gateway mode, which always sends `--tools ""`.
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    /// claude-code only: `--allowed-tools`. `None`/absent => flag omitted.
    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,
    /// claude-code only: `--disallowed-tools`. `None`/absent => flag omitted.
    #[serde(default)]
    pub disallowed_tools: Option<Vec<String>>,
    /// claude-code only: per-turn wall-clock limit in seconds. On expiry the
    /// child is killed and the turn ends with an error, leaving the provider
    /// usable for the next turn. `None`/absent => 900.
    #[serde(default)]
    pub turn_timeout_secs: Option<u64>,
    /// claude-code only: `--permission-mode`. Passed through unvalidated;
    /// Claude Code rejects unknown values itself. `None`/absent => omitted.
    #[serde(default)]
    pub permission_mode: Option<String>,
    /// claude-code only: `--max-budget-usd`. `None`/absent => flag omitted.
    #[serde(default)]
    pub max_budget_usd: Option<f64>,
    /// claude-code only: how the persona reaches Claude Code. `"append"`
    /// (default) uses `--append-system-prompt`; `"replace"` uses
    /// `--system-prompt`. Unknown values are rejected.
    #[serde(default)]
    pub system_prompt_mode: Option<String>,
    /// claude-code only: extra CLI arguments appended verbatim before the
    /// prompt argument. Escape hatch for flags this config does not model.
    #[serde(default)]
    pub extra_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub auto_approve_tools: bool,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    #[serde(default)]
    pub registry_dir: String,
    #[serde(default = "default_agents_dir")]
    pub agents_dir: String,
    #[serde(default)]
    pub persona_file: String,
    /// Maximum tool-use iterations per peer prompt. Each iteration is one
    /// (LLM call → optional tool calls + their results) round. The default
    /// of 8 is too low for design-then-debug workflows where the agent
    /// generates artifacts, runs validators, and iterates on lint feedback.
    /// Bump to 16+ for orchestrators that own multiple tools per turn.
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: u32,
    /// Directory of user-defined custom slash commands (`*.md` files), resolved
    /// relative to the working directory unless absolute / `~`-expanded. Empty
    /// string falls back to the default (`.agent-cli/commands`). See FR-14.
    #[serde(default = "default_commands_dir")]
    pub commands_dir: String,
    /// Default group id for agents this config launches. Empty / unset means no
    /// group unless `--group` is given on the command line. Detached children
    /// inherit their launcher's effective group.
    #[serde(default)]
    pub group: Option<String>,
}

fn default_max_tool_iterations() -> u32 {
    24
}

fn default_commands_dir() -> String {
    ".agent-cli/commands".to_string()
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            auto_approve_tools: false,
            log_dir: default_log_dir(),
            registry_dir: String::new(),
            agents_dir: default_agents_dir(),
            persona_file: String::new(),
            max_tool_iterations: default_max_tool_iterations(),
            commands_dir: default_commands_dir(),
            group: None,
        }
    }
}

fn default_log_dir() -> String {
    "~/.local/share/agent-cli/logs".to_string()
}

fn default_agents_dir() -> String {
    "~/.config/agent-cli/agents".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_tools_enabled")]
    pub enabled: Vec<String>,
    /// Bash tool tuning. `#[serde(alias = "shell")]` keeps the legacy
    /// `[tools.shell]` block loadable after the rename (its `timeout_secs`
    /// field has no equivalent and is ignored; `timeout_ms` falls back to the
    /// default).
    #[serde(default, alias = "shell")]
    pub bash: BashToolConfig,
    #[serde(default)]
    pub websearch: WebSearchConfig,
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: default_tools_enabled(),
            bash: BashToolConfig::default(),
            websearch: WebSearchConfig::default(),
        }
    }
}

fn default_tools_enabled() -> Vec<String> {
    vec![
        "bash".to_string(),
        "read".to_string(),
        "write".to_string(),
        "send_to".to_string(),
        // Managing the agents this one created: listing is read-only, and
        // stopping cannot reach outside the caller's own tree, so both are on
        // by default. Creating agents (`spawn`) stays opt-in.
        "list_agents".to_string(),
        "stop_agent".to_string(),
        "monitor".to_string(),
        "edit".to_string(),
        "glob".to_string(),
        "grep".to_string(),
        "websearch".to_string(),
        "webfetch".to_string(),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashToolConfig {
    /// Default execution timeout for the `bash` tool, in milliseconds.
    #[serde(default = "default_bash_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_bash_max_output")]
    pub max_output_kb: u64,
}

impl Default for BashToolConfig {
    fn default() -> Self {
        Self {
            timeout_ms: default_bash_timeout_ms(),
            max_output_kb: default_bash_max_output(),
        }
    }
}

fn default_bash_timeout_ms() -> u64 {
    120_000
}

fn default_bash_max_output() -> u64 {
    256
}

/// Configuration for the `websearch` tool. Network tools are opt-in: when
/// `api_key_env` / `endpoint` are absent, `websearch` returns a clear error.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebSearchConfig {
    /// Environment variable holding the search API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Search endpoint URL (provider-specific).
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Search provider identifier (e.g. "tavily", "brave"). Default: "tavily".
    #[serde(default)]
    pub provider: Option<String>,
}

/// Model Context Protocol (MCP) client configuration. agent-cli connects to the
/// declared servers at startup (stdio transport), discovers their tools, and
/// registers each as `mcp__<server>__<tool>`. Omitting the section (no servers)
/// leaves behaviour unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Declared MCP servers (`[[mcp.servers]]`).
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
    /// Per-server handshake + `tools/list` timeout in milliseconds. Applied at
    /// connect time; defaults to `default_mcp_init_timeout_ms()` when unset.
    #[serde(default)]
    pub init_timeout_ms: Option<u64>,
}

/// A single MCP server. Reached over `stdio` (a launched subprocess, the
/// default) or `http` (Streamable HTTP to `url`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Logical name; used in the tool namespace `mcp__<name>__<tool>`.
    pub name: String,
    /// Executable to launch (resolved on PATH or an absolute path). stdio only.
    #[serde(default)]
    pub command: String,
    /// Arguments passed to `command`. stdio only.
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables merged onto the inherited environment. stdio only.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Working directory for the child (subject to `~`/env expansion). stdio only.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Whether this server is connected. Default: true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Transport kind: "stdio" (default) or "http" (Streamable HTTP).
    #[serde(default)]
    pub transport: Option<String>,
    /// HTTP endpoint URL (required when `transport = "http"`). http only.
    #[serde(default)]
    pub url: Option<String>,
    /// Static request headers sent on every HTTP call. http only.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Env var whose value is sent as `Authorization: Bearer <value>`. http only.
    #[serde(default)]
    pub api_key_env: Option<String>,
}

/// Default handshake/list timeout for an MCP server (milliseconds).
pub fn default_mcp_init_timeout_ms() -> u64 {
    15_000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_show_thinking")]
    pub show_thinking: String,
    /// Draw the progress indicator (activity line + spinner and elapsed time)
    /// while a turn runs. Ignored when the output is not an interactive
    /// terminal, where the indicator is never drawn.
    #[serde(default = "default_show_progress")]
    pub show_progress: bool,
    /// Colour the terminal output: `"auto"` (colour a stream when it is an
    /// interactive terminal and `NO_COLOR` is unset), `"always"`, `"never"`.
    #[serde(default = "default_color")]
    pub color: String,
    /// Scroll the session log with the mouse wheel, keeping the prompt line
    /// pinned where it is. While this is on the terminal reports wheel, click
    /// and drag events to agent-cli: the wheel scrolls, dragging selects the
    /// log (`mouse_select`), and the terminal's own scrollback and selection
    /// stay available under its usual `Shift` override. Turning it off hands
    /// the mouse back to the terminal entirely.
    #[serde(default = "default_mouse_scroll")]
    pub mouse_scroll: bool,
    /// Lines of session output kept for scrolling back. `0` keeps none, which
    /// also disables the wheel scrollback.
    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,
    /// Select a range of the session log by dragging with the left mouse
    /// button, and copy it on release. Only has an effect while `mouse_scroll`
    /// is on — that is what puts the mouse in agent-cli's hands.
    #[serde(default = "default_mouse_select")]
    pub mouse_select: bool,
    /// Command the selected text is piped to instead of being written to the
    /// terminal as OSC 52 — e.g. `"wl-copy"` or `"xclip -selection clipboard"`.
    /// Empty (the default) uses OSC 52, which also works over SSH.
    #[serde(default)]
    pub copy_command: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_thinking: default_show_thinking(),
            show_progress: default_show_progress(),
            color: default_color(),
            mouse_scroll: default_mouse_scroll(),
            scrollback_lines: default_scrollback_lines(),
            mouse_select: default_mouse_select(),
            copy_command: String::new(),
        }
    }
}

fn default_show_thinking() -> String {
    "collapsed".to_string()
}

fn default_show_progress() -> bool {
    true
}

fn default_color() -> String {
    "auto".to_string()
}

fn default_mouse_select() -> bool {
    true
}

fn default_mouse_scroll() -> bool {
    true
}

fn default_scrollback_lines() -> usize {
    2000
}

/// `[shell]` — running a command typed at the prompt with `!<command>`.
///
/// This is the user's own command, not a tool call the model asked for, so it
/// is not subject to the approval gate or to `[tools] enabled`; `enabled`
/// below is its only switch. The timeout and the output cap default to the
/// same values as `[tools.bash]`, so the two execution paths behave alike.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    /// Run `!<command>` typed at the prompt. When false, such a line is sent
    /// to the model as an ordinary prompt.
    #[serde(default = "default_shell_enabled")]
    pub enabled: bool,
    /// Give up on a command after this long and kill it.
    #[serde(default = "default_shell_timeout_ms")]
    pub timeout_ms: u64,
    /// How much of the output is handed to the model. The screen always shows
    /// all of it; this bounds only the copy that enters the conversation.
    #[serde(default = "default_shell_max_output_kb")]
    pub max_output_kb: u64,
    /// Hand the command and its output to the model as context, so the next
    /// question can refer to it without pasting.
    #[serde(default = "default_shell_context")]
    pub context: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            enabled: default_shell_enabled(),
            timeout_ms: default_shell_timeout_ms(),
            max_output_kb: default_shell_max_output_kb(),
            context: default_shell_context(),
        }
    }
}

fn default_shell_enabled() -> bool {
    true
}

fn default_shell_timeout_ms() -> u64 {
    120_000
}

fn default_shell_max_output_kb() -> u64 {
    256
}

fn default_shell_context() -> bool {
    true
}

/// `[spawn]` — how far an agent may go in creating agents of its own.
///
/// These bound the **`spawn` tool**, i.e. the path the model takes on its own.
/// `agent-cli spawn` and the REPL's `/spawn` are a person deciding and are not
/// bounded; their behaviour is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Live direct children one agent may have. 0 disables autonomous spawning.
    #[serde(default = "default_max_children")]
    pub max_children: u32,
    /// How deep a chain of tool-spawned agents may go: with 2, a root spawns a
    /// child and that child spawns a grandchild, which may not spawn further.
    /// 0 also disables autonomous spawning.
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
}

impl Default for SpawnConfig {
    fn default() -> Self {
        Self {
            max_children: default_max_children(),
            max_depth: default_max_depth(),
        }
    }
}

fn default_max_children() -> u32 {
    4
}

fn default_max_depth() -> u32 {
    2
}

/// `[history]` — hybrid history-window management. Opt-in (`enabled = false`
/// by default); when disabled the conversation is replayed verbatim as before.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    /// Master switch. When false, no summarization or trimming occurs.
    #[serde(default)]
    pub enabled: bool,
    /// Approximate context budget (estimated tokens ≈ chars/4). When the
    /// estimated history exceeds this, compaction runs.
    #[serde(default = "default_max_context_tokens")]
    pub max_context_tokens: usize,
    /// Number of most-recent turns always kept verbatim (never summarized or
    /// dropped). Leading system/persona messages are always kept too.
    #[serde(default = "default_keep_recent_turns")]
    pub keep_recent_turns: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_context_tokens: default_max_context_tokens(),
            keep_recent_turns: default_keep_recent_turns(),
        }
    }
}

fn default_max_context_tokens() -> usize {
    24_000
}

fn default_keep_recent_turns() -> usize {
    6
}

/// Display mode for `[ui] show_thinking` (FR-03-1-2 / design doc 4.3C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowThinkingMode {
    /// Do not show thinking at all.
    Hidden,
    /// Show up to the first 80 characters of each delta, truncating the rest with `...`.
    Collapsed,
    /// Show the full text as received.
    Expanded,
}

impl UiConfig {
    /// Normalize the `show_thinking` string into a `ShowThinkingMode`. Unknown values
    /// fall back to the default `Collapsed` (does not prevent startup on parse errors).
    pub fn show_thinking_mode(&self) -> ShowThinkingMode {
        match self.show_thinking.as_str() {
            "hidden" => ShowThinkingMode::Hidden,
            "expanded" => ShowThinkingMode::Expanded,
            "collapsed" => ShowThinkingMode::Collapsed,
            _ => ShowThinkingMode::Collapsed,
        }
    }

    /// Normalize the `color` string into a [`ColorMode`]. Unknown values fall
    /// back to the default `Auto`, like `show_thinking` above.
    pub fn color_mode(&self) -> ColorMode {
        ColorMode::parse(&self.color)
    }
}

/// The ordered chain of config files that make up the effective configuration,
/// **lowest priority first**. Never empty.
///
/// Configuration is layered: the user-level file is the base and a project-local
/// `.agent-cli/config.toml` is the overlay that wins key by key. Before this was
/// a chain, a project-local file *replaced* the user-level one outright, so
/// every key the project did not mention silently reverted to a built-in
/// default. See [`merge`] for how two layers combine.
#[derive(Debug, Clone, Default)]
pub struct ConfigSource {
    /// Lowest priority first.
    pub layers: Vec<PathBuf>,
    pub from_explicit: bool,
}

impl ConfigSource {
    /// The highest-priority layer: the file `config edit` opens and the one a
    /// message means when it says "the config file".
    ///
    /// Falls back to the user-level default path for a default-constructed
    /// `ConfigSource` (tests), which has no layers.
    pub fn path(&self) -> PathBuf {
        self.layers
            .last()
            .cloned()
            .or_else(|| default_path().ok())
            .unwrap_or_default()
    }

    /// Every layer, lowest priority first — what a detached child must be given
    /// so it reconstructs the same configuration its parent is running under.
    pub fn chain(&self) -> &[PathBuf] {
        &self.layers
    }
}

pub fn default_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| AppError::config("could not resolve user config directory"))?;
    Ok(base.join("agent-cli").join("config.toml"))
}

/// Project-local config file: `.agent-cli/config.toml` under the current
/// working directory. Returns it only when it exists, resolved to an absolute
/// path so a detached child launched with `--config <path>` reads the same file
/// regardless of the working directory it inherits.
pub fn local_path() -> Option<PathBuf> {
    local_path_in(&std::env::current_dir().ok()?)
}

/// Project-local config path under `dir`, returned only when it exists.
fn local_path_in(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(".agent-cli").join("config.toml");
    path.is_file().then_some(path)
}

/// Resolve the chain of config files to load, lowest priority first.
///
/// With no `--config`, the chain is the user-level
/// `~/.config/agent-cli/config.toml` followed by a project-local
/// `.agent-cli/config.toml` when one exists — so the project file overlays the
/// user file rather than hiding it.
///
/// `--config <path>` replaces the chain entirely: one occurrence means that
/// file alone, which is what keeps the flag usable for an isolated run. Several
/// occurrences layer left to right, which is how a detached child is handed the
/// chain its parent resolved.
pub fn resolve_path(explicit: &[PathBuf]) -> Result<ConfigSource> {
    if !explicit.is_empty() {
        let mut layers = Vec::with_capacity(explicit.len());
        for p in explicit {
            layers.push(expand_path(p.to_string_lossy().as_ref())?);
        }
        return Ok(ConfigSource {
            layers,
            from_explicit: true,
        });
    }
    let mut layers = vec![default_path()?];
    if let Some(path) = local_path() {
        layers.push(path);
    }
    Ok(ConfigSource {
        layers,
        from_explicit: false,
    })
}

/// Merge `overlay` onto `base`, `overlay` winning.
///
/// - **Tables merge recursively.** A project file that sets `[provider] kind`
///   must not discard the user's `[ui]`, `[runtime]` or `[provider.claude]`.
/// - **Scalars and arrays are replaced.** `[tools] enabled` in a project file
///   means "this tool set, here"; appending would make it impossible to
///   *narrow* a tool set locally, which is the more common intent.
/// - **`permissions.deny` and `permissions.allow` are unioned**, against the
///   array rule. Under replacement a project file could delete a user's deny
///   list simply by redefining it, and a project directory is exactly the thing
///   a user may not have written themselves. An `allow` can never beat a `deny`,
///   so unioning allow lists cannot widen the boundary either way.
pub fn merge(base: toml::Value, overlay: toml::Value) -> toml::Value {
    merge_at(base, overlay, &[])
}

fn merge_at(base: toml::Value, overlay: toml::Value, path: &[&str]) -> toml::Value {
    use toml::Value;
    match (base, overlay) {
        (Value::Table(mut b), Value::Table(o)) => {
            for (k, ov) in o {
                let next: Vec<&str> = path.iter().copied().chain(std::iter::once(k.as_str())).collect();
                let merged = match b.remove(&k) {
                    Some(bv) => merge_at(bv, ov, &next),
                    None => ov,
                };
                b.insert(k, merged);
            }
            Value::Table(b)
        }
        (Value::Array(b), Value::Array(o)) if unions(path) => {
            let mut out = b;
            for item in o {
                if !out.contains(&item) {
                    out.push(item);
                }
            }
            Value::Array(out)
        }
        (_, overlay) => overlay,
    }
}

/// The two arrays that union rather than replace when layers combine.
fn unions(path: &[&str]) -> bool {
    matches!(path, ["permissions", "deny"] | ["permissions", "allow"])
}

/// The TOML spellings that deserialize to one field, and the canonical one.
///
/// Keep in step with the `#[serde(alias = ...)]` attributes on the config
/// structs — serde rejects a table carrying both spellings of one field, and
/// layering two files can produce exactly that: the merge matches keys by name
/// on `toml::Value`, so a base `[tools.bash]` plus an overlay `[tools.shell]`
/// deserialize as one table holding both (see doc/troubleshooting.md).
const ALIASES: &[(&str, &str, &str)] = &[
    // (table, legacy, canonical)
    ("tools", "shell", "bash"),
    ("permissions", "defaultMode", "default_mode"),
];

/// Rename a layer's legacy spellings to their canonical ones.
///
/// Applied to each layer right after its parse and before it enters
/// [`merge`], so layer precedence — the overlay winning — decides the outcome
/// for free, whatever spelling each layer used. Within one layer carrying both
/// spellings there is no order to consult, so the two tables combine: the
/// canonical spelling's keys win each conflict, the legacy table's keys fill
/// where the canonical one is absent. Every rename is reported, naming the
/// file — a rewrite nobody hears about is not a compatibility shim but a
/// surprise.
///
/// Only the aliased keys are touched; every other key passes through. The
/// returned strings are the report: one line per renamed key.
fn canonicalize_aliases(value: toml::Value, file: &Path) -> (toml::Value, Vec<String>) {
    use toml::Value;

    let mut renames = Vec::new();
    let mut value = value;
    let Value::Table(root) = &mut value else {
        return (value, renames);
    };
    for (table, legacy, canonical) in ALIASES {
        let Some(Value::Table(t)) = root.get_mut(*table) else {
            continue;
        };
        let legacy_table = t.remove(*legacy);
        let canonical_table = t.remove(*canonical);
        if legacy_table.is_none() && canonical_table.is_none() {
            continue;
        }
        let combined = match (legacy_table, canonical_table) {
            (Some(Value::Table(lt)), Some(Value::Table(mut ct))) => {
                for (k, v) in lt {
                    ct.entry(k).or_insert(v);
                }
                Value::Table(ct)
            }
            // A legacy value that is not a table cannot be merged into one:
            // the canonical spelling wins the conflict.
            (Some(_), Some(c)) => c,
            (Some(l), None) => l,
            (None, Some(c)) => c,
            (None, None) => continue,
        };
        renames.push(format!(
            "renamed legacy key [{table}.{legacy}] → [{table}.{canonical}] in {}",
            file.display()
        ));
        t.insert((*canonical).to_string(), combined);
    }
    (value, renames)
}

/// Read and merge every layer of `source`.
///
/// The merge happens on `toml::Value`, before `Config` is deserialized, so
/// every section gets the layering behaviour without its own merge code.
///
/// `DEFAULT_CONFIG` is written out **only when no layer exists at all** and none
/// was explicit. A project-local file must not cause a user-level file to be
/// created, and a `[permissions]` section must never appear that nobody wrote —
/// which is why the block `DEFAULT_CONFIG` ships is commented out.
pub fn load(source: &ConfigSource) -> Result<Config> {
    let present: Vec<&PathBuf> = source.layers.iter().filter(|p| p.exists()).collect();

    if present.is_empty() {
        if source.from_explicit {
            return Err(AppError::ConfigNotFound(source.path()));
        }
        let target = source
            .layers
            .first()
            .cloned()
            .ok_or_else(|| AppError::config("no configuration path to generate"))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, DEFAULT_CONFIG)?;
        tracing::info!(path = %target.display(), "default config generated");
        let cfg: Config = toml::from_str(DEFAULT_CONFIG)?;
        return Ok(cfg);
    }

    let mut merged: Option<toml::Value> = None;
    for path in present {
        let raw = std::fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&raw).map_err(|e| {
            AppError::config(format!("{}: {e}", path.display()))
        })?;
        let (value, renames) = canonicalize_aliases(value, path);
        for rename in &renames {
            tracing::info!(file = %path.display(), rename, "renamed legacy key to its canonical spelling");
        }
        merged = Some(match merged {
            Some(base) => merge(base, value),
            None => value,
        });
    }
    let merged = merged.expect("at least one layer was present");
    let chain = source
        .chain()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let cfg: Config = merged.try_into().map_err(|e| {
        AppError::config(format!("merged configuration ({chain}): {e}"))
    })?;
    Ok(cfg)
}

pub fn expand_path(p: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(p)
        .map_err(|e| AppError::config(format!("path expansion failed: {e}")))?;
    Ok(PathBuf::from(expanded.into_owned()))
}

/// Format an API key value for masked display (FR-09-3).
///
/// Keys of 8 or more characters are shown as "first 4 chars + `...` + last 4 chars".
/// Shorter keys return `***` to avoid leaking the value length.
/// Empty strings return an empty string (callers should check `Option` first to
/// distinguish "not set").
pub fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 8 {
        return "***".to_string();
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}...{tail}")
}

impl Config {
    pub fn provider_entry(&self, kind: &str) -> Option<&ProviderEntry> {
        match kind {
            "claude" => self.provider.claude.as_ref(),
            "claude-code" => self.provider.claude_code.as_ref(),
            "codex" => self.provider.codex.as_ref(),
            "ollama" => self.provider.ollama.as_ref(),
            "opencode" | "opencode-go" => self.provider.opencode.as_ref(),
            "llama.cpp" => self.provider.llamacpp.as_ref(),
            _ => None,
        }
    }

    /// Effective group for a launch: CLI `--group` wins, else `[runtime] group`,
    /// else `None`. Empty strings are treated as unset.
    pub fn resolve_group(&self, cli_group: Option<&str>) -> Option<crate::id::GroupId> {
        cli_group
            .or(self.runtime.group.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| crate::id::GroupId(s.to_string()))
    }

    pub fn apply_overrides(&mut self, provider: Option<&str>, model: Option<&str>) {
        if let Some(p) = provider {
            self.provider.kind = p.to_string();
        }
        if let Some(m) = model {
            if let Some(entry) = self.provider_entry_mut(&self.provider.kind.clone()) {
                entry.model = Some(m.to_string());
            }
        }
    }

    pub fn apply_persona_overrides(&mut self, model: Option<&str>, temperature: Option<f32>) {
        let kind = self.provider.kind.clone();
        if let Some(entry) = self.provider_entry_mut(&kind) {
            if let Some(m) = model {
                entry.model = Some(m.to_string());
            }
            if let Some(t) = temperature {
                entry.temperature = Some(t);
            }
        }
    }

    fn provider_entry_mut(&mut self, kind: &str) -> Option<&mut ProviderEntry> {
        match kind {
            "claude" => Some(self.provider.claude.get_or_insert_with(Default::default)),
            "claude-code" => Some(
                self.provider
                    .claude_code
                    .get_or_insert_with(Default::default),
            ),
            "codex" => Some(self.provider.codex.get_or_insert_with(Default::default)),
            "ollama" => Some(self.provider.ollama.get_or_insert_with(Default::default)),
            "opencode" | "opencode-go" => Some(self.provider.opencode.get_or_insert_with(Default::default)),
            "llama.cpp" => Some(self.provider.llamacpp.get_or_insert_with(Default::default)),
            _ => None,
        }
    }

    pub fn registry_dir(&self) -> Result<PathBuf> {
        if !self.runtime.registry_dir.is_empty() {
            return expand_path(&self.runtime.registry_dir);
        }
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !dir.is_empty() {
                return Ok(PathBuf::from(dir).join("agent-cli"));
            }
        }
        Ok(PathBuf::from("/tmp/agent-cli"))
    }

    pub fn log_dir(&self) -> Result<PathBuf> {
        expand_path(&self.runtime.log_dir)
    }

    pub fn agents_dir(&self) -> Result<PathBuf> {
        expand_path(&self.runtime.agents_dir)
    }

    /// When `provider.kind` is `"opencode-go"`, apply Go-specific defaults to
    /// the `[provider.opencode]` entry (filling `None` fields) and normalize
    /// the kind to `"opencode"`. When the kind is not `"opencode-go"`, this is
    /// a no-op.
    ///
    /// The Go endpoint serves open-weight models only — no `claude-*` id is
    /// available there (those live on `https://opencode.ai/zen/v1`), and their
    /// ids are published in OpenAI shape. `model` is one confirmed served by
    /// `GET https://opencode.ai/zen/go/v1/models` on 2026-09-13; the catalogue
    /// is the vendor's, so check that endpoint if a model error appears.
    pub fn apply_opencode_go_defaults(&mut self) {
        if self.provider.kind != "opencode-go" {
            return;
        }
        let entry = self.provider.opencode.get_or_insert_with(Default::default);
        if entry.base_url.is_none() {
            entry.base_url = Some("https://opencode.ai/zen/go/v1".to_string());
        }
        if entry.api.is_none() {
            entry.api = Some("openai".to_string());
        }
        if entry.model.is_none() {
            entry.model = Some("qwen3.8-max".to_string());
        }
        if entry.api_key_env.is_none() {
            entry.api_key_env = Some("OPENCODE_API_KEY".to_string());
        }
        self.provider.kind = "opencode".to_string();
    }
}

#[cfg(test)]
pub(crate) fn tests_default_config() -> &'static str {
    DEFAULT_CONFIG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_selection_keys_default_to_on_and_the_terminals_own_clipboard() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(cfg.ui.mouse_select, "selecting the log is on by default");
        assert_eq!(cfg.ui.copy_command, "", "OSC 52 unless a command is named");
        // Absent from an older config file: the defaults still apply.
        let bare: Config =
            toml::from_str("[provider]\nkind = \"claude\"\n\n[ui]\nmouse_scroll = true\n").unwrap();
        assert!(bare.ui.mouse_select);
        assert_eq!(bare.ui.copy_command, "");
    }

    #[test]
    fn ui_selection_keys_parse() {
        let cfg: Config = toml::from_str(
            "[provider]\nkind = \"claude\"\n\n[ui]\nmouse_select = false\ncopy_command = \"wl-copy\"\n",
        )
        .unwrap();
        assert!(!cfg.ui.mouse_select);
        assert_eq!(cfg.ui.copy_command, "wl-copy");
    }

    #[test]
    fn local_path_in_found_only_when_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        // No `.agent-cli/config.toml` yet.
        assert!(local_path_in(dir.path()).is_none());
        // A bare `.agent-cli` dir (no config file) is still not a match.
        std::fs::create_dir(dir.path().join(".agent-cli")).unwrap();
        assert!(local_path_in(dir.path()).is_none());
        // The file existing makes it a match, returning that exact path.
        let cfg = dir.path().join(".agent-cli").join("config.toml");
        std::fs::write(&cfg, "[provider]\nkind = \"claude\"\n").unwrap();
        assert_eq!(local_path_in(dir.path()), Some(cfg));
    }

    #[test]
    fn one_explicit_config_replaces_the_whole_chain() {
        // A single `--config` must still mean "that file alone" — it is how an
        // isolated run is obtained, and layering a user file underneath it
        // would take that away.
        let src = resolve_path(&[PathBuf::from("/etc/agent-cli/custom.toml")]).unwrap();
        assert_eq!(
            src.chain(),
            [PathBuf::from("/etc/agent-cli/custom.toml")].as_slice()
        );
        assert_eq!(src.path(), PathBuf::from("/etc/agent-cli/custom.toml"));
        assert!(src.from_explicit);
    }

    #[test]
    fn repeated_explicit_configs_layer_left_to_right() {
        // This is how a detached child is handed the chain its parent resolved.
        let src = resolve_path(&[PathBuf::from("/a/base.toml"), PathBuf::from("/b/over.toml")])
            .unwrap();
        assert_eq!(
            src.chain(),
            [PathBuf::from("/a/base.toml"), PathBuf::from("/b/over.toml")].as_slice()
        );
        assert_eq!(src.path(), PathBuf::from("/b/over.toml"), "the last one wins");
        assert!(src.from_explicit);
    }

    #[test]
    fn the_default_chain_starts_at_the_user_file() {
        // Without `--config` the user-level file is always the base layer, even
        // when it does not exist yet; a project file is added on top of it
        // rather than replacing it.
        let src = resolve_path(&[]).unwrap();
        assert!(!src.from_explicit);
        assert_eq!(src.chain().first(), Some(&default_path().unwrap()));
        assert!(!src.chain().is_empty());
    }

    // --- V-3 / V-4: how two layers combine ----------------------------------

    fn merged(base: &str, overlay: &str) -> Config {
        let b: toml::Value = toml::from_str(base).unwrap();
        let o: toml::Value = toml::from_str(overlay).unwrap();
        merge(b, o).try_into().unwrap()
    }

    #[test]
    fn a_project_layer_overrides_key_by_key_without_discarding_the_rest() {
        // The whole point of layering: before it, a project file that set one
        // key silently reverted every other key to a built-in default.
        let cfg = merged(
            r#"
[provider]
kind = "claude"
[provider.claude]
model = "claude-opus-4-7"
thinking = true
[ui]
color = "always"
[runtime]
auto_approve_tools = true
"#,
            r#"
[provider]
kind = "ollama"
"#,
        );
        assert_eq!(cfg.provider.kind, "ollama", "the overlay wins");
        assert_eq!(cfg.ui.color, "always", "an untouched section survives");
        assert!(cfg.runtime.auto_approve_tools, "so does an untouched key");
        let claude = cfg.provider.claude.expect("nested table survives");
        assert_eq!(claude.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[test]
    fn nested_tables_merge_rather_than_replace() {
        let cfg = merged(
            r#"
[provider]
kind = "claude"
[provider.claude]
model = "claude-opus-4-7"
base_url = "https://api.anthropic.com"
"#,
            r#"
[provider.claude]
model = "claude-sonnet-4-5"
"#,
        );
        let claude = cfg.provider.claude.unwrap();
        assert_eq!(claude.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(
            claude.base_url.as_deref(),
            Some("https://api.anthropic.com"),
            "the key the overlay did not mention is kept"
        );
    }

    #[test]
    fn an_array_is_replaced_by_the_overlay() {
        // `[tools] enabled` in a project file means "this tool set, here".
        // Appending would make it impossible to narrow a tool set locally.
        let cfg = merged(
            "[provider]\nkind = \"claude\"\n[tools]\nenabled = [\"bash\", \"read\", \"write\"]\n",
            "[tools]\nenabled = [\"read\"]\n",
        );
        assert_eq!(cfg.tools.enabled, vec!["read".to_string()]);
    }

    #[test]
    fn permission_lists_union_so_a_project_cannot_delete_a_users_deny() {
        // The one exception to the array rule, and the reason for it: under
        // replacement a project directory — which the user may not have written
        // — could switch off a machine-wide deny just by redefining the list.
        let cfg = merged(
            r#"
[provider]
kind = "claude"
[permissions]
deny = ["bash(rm -rf ~/**)"]
allow = ["bash(git:*)"]
"#,
            r#"
[permissions]
deny = ["bash(curl:*)"]
allow = ["read(**)"]
"#,
        );
        assert!(
            cfg.permissions.deny.contains(&"bash(rm -rf ~/**)".to_string()),
            "the user's deny survives the project layer: {:?}",
            cfg.permissions.deny
        );
        assert!(cfg.permissions.deny.contains(&"bash(curl:*)".to_string()));
        assert_eq!(cfg.permissions.allow.len(), 2);
    }

    #[test]
    fn a_union_does_not_duplicate_a_rule_both_layers_wrote() {
        let cfg = merged(
            "[provider]\nkind = \"claude\"\n[permissions]\ndeny = [\"read(.env*)\"]\n",
            "[permissions]\ndeny = [\"read(.env*)\", \"bash(rm:*)\"]\n",
        );
        assert_eq!(cfg.permissions.deny.len(), 2, "{:?}", cfg.permissions.deny);
    }

    #[test]
    fn the_project_layer_sets_the_default_mode() {
        let cfg = merged(
            "[provider]\nkind = \"claude\"\n[permissions]\ndefault_mode = \"ask\"\n",
            "[permissions]\ndefault_mode = \"deny\"\n",
        );
        assert_eq!(cfg.permissions.default_mode.as_deref(), Some("deny"));
    }

    // --- V-6: nothing is generated that nobody asked for ---------------------

    #[test]
    fn a_project_file_alone_does_not_create_a_user_file() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user").join("config.toml");
        let project = dir.path().join("project").join("config.toml");
        std::fs::create_dir_all(project.parent().unwrap()).unwrap();
        std::fs::write(&project, "[provider]\nkind = \"ollama\"\n").unwrap();

        let source = ConfigSource {
            layers: vec![user.clone(), project],
            from_explicit: false,
        };
        let cfg = load(&source).unwrap();
        assert_eq!(cfg.provider.kind, "ollama");
        assert!(
            !user.exists(),
            "a missing base layer must not be conjured into existence"
        );
    }

    #[test]
    fn the_default_config_is_written_only_when_no_layer_exists() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("config.toml");
        let source = ConfigSource {
            layers: vec![user.clone()],
            from_explicit: false,
        };
        let cfg = load(&source).unwrap();
        assert!(user.exists(), "the first run still gets a config file");
        assert_eq!(cfg.provider.kind, "claude");
    }

    #[test]
    fn the_shipped_permissions_block_is_commented_out() {
        // A permission rule nobody wrote is exactly the mistake this cycle
        // removed from the repository; the default config must not reintroduce
        // it by shipping live rules.
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(cfg.permissions.deny.is_empty());
        assert!(cfg.permissions.allow.is_empty());
        assert!(cfg.permissions.default_mode.is_none());
        assert!(
            DEFAULT_CONFIG.contains("# [permissions]"),
            "the syntax should still be documented by example"
        );
    }

    #[test]
    fn a_missing_explicit_config_is_still_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let source = ConfigSource {
            layers: vec![dir.path().join("nope.toml")],
            from_explicit: true,
        };
        assert!(matches!(
            load(&source),
            Err(AppError::ConfigNotFound(_))
        ));
    }

    #[test]
    fn a_layer_that_does_not_parse_names_the_file_it_came_from() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("broken.toml");
        std::fs::write(&bad, "[provider\nkind = ").unwrap();
        let source = ConfigSource {
            layers: vec![bad.clone()],
            from_explicit: true,
        };
        let err = load(&source).unwrap_err().to_string();
        assert!(
            err.contains("broken.toml"),
            "the user has to be told which layer is broken: {err}"
        );
    }

    #[test]
    fn permissions_default_to_an_empty_section() {
        let cfg: Config = toml::from_str("[provider]\nkind = \"claude\"\n").unwrap();
        assert!(cfg.permissions.deny.is_empty());
        assert!(cfg.permissions.allow.is_empty());
        assert!(cfg.permissions.default_mode.is_none());
    }

    #[test]
    fn parse_default_config() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).expect("default config must parse");
        assert_eq!(cfg.provider.kind, "claude");
        assert!(cfg.provider.claude.is_some());
        assert!(cfg.provider.ollama.is_some());
        assert!(cfg.provider.llamacpp.is_some());
        assert_eq!(cfg.tools.enabled.len(), 12);
        assert_eq!(cfg.tools.bash.timeout_ms, 120_000);
        assert_eq!(cfg.runtime.commands_dir, ".agent-cli/commands");
    }

    #[test]
    fn runtime_commands_dir_default() {
        assert_eq!(RuntimeConfig::default().commands_dir, ".agent-cli/commands");
    }

    #[test]
    fn default_config_has_no_mcp_servers() {
        // The shipped default only comments out [mcp], so the section is empty.
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert!(cfg.mcp.servers.is_empty());
        assert!(cfg.mcp.init_timeout_ms.is_none());
    }

    #[test]
    fn parses_mcp_servers_with_defaults() {
        let toml_src = r#"
[provider]
kind = "claude"

[mcp]
init_timeout_ms = 9000

[[mcp.servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
env = { EXAMPLE = "1" }
cwd = "/tmp"

[[mcp.servers]]
name = "disabled-one"
command = "foo"
enabled = false
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.mcp.init_timeout_ms, Some(9000));
        assert_eq!(cfg.mcp.servers.len(), 2);

        let fs = &cfg.mcp.servers[0];
        assert_eq!(fs.name, "filesystem");
        assert_eq!(fs.command, "npx");
        assert_eq!(fs.args.len(), 3);
        assert_eq!(fs.env.get("EXAMPLE").map(String::as_str), Some("1"));
        assert_eq!(fs.cwd.as_deref(), Some("/tmp"));
        assert!(fs.enabled, "enabled defaults to true");
        assert!(fs.transport.is_none());

        let disabled = &cfg.mcp.servers[1];
        assert!(!disabled.enabled);
        assert!(disabled.args.is_empty());
    }

    #[test]
    fn parses_http_mcp_server() {
        let toml_src = r#"
[provider]
kind = "claude"

[[mcp.servers]]
name        = "remote"
transport   = "http"
url         = "https://example.com/mcp"
headers     = { X-Example = "1" }
api_key_env = "REMOTE_MCP_TOKEN"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.mcp.servers.len(), 1);
        let s = &cfg.mcp.servers[0];
        assert_eq!(s.transport.as_deref(), Some("http"));
        assert_eq!(s.url.as_deref(), Some("https://example.com/mcp"));
        assert_eq!(s.headers.get("X-Example").map(String::as_str), Some("1"));
        assert_eq!(s.api_key_env.as_deref(), Some("REMOTE_MCP_TOKEN"));
        // http servers need no command; it defaults to empty.
        assert!(s.command.is_empty());
        assert!(s.enabled);
    }

    #[test]
    fn config_with_custom_commands_dir() {
        let toml_src = r#"
[provider]
kind = "claude"
[runtime]
commands_dir = "/tmp/cmds"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.runtime.commands_dir, "/tmp/cmds");
    }

    #[test]
    fn config_without_commands_dir_uses_default() {
        let toml_src = r#"
[provider]
kind = "claude"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(cfg.runtime.commands_dir, ".agent-cli/commands");
    }

    /// FR-04-3 boundary (upper limit): TOML with `max_tool_iterations` set to `u32::MAX`
    /// parses successfully and the value is preserved. We don't actually loop 4 billion
    /// times (parse success alone guarantees that true unlimited is disallowed but
    /// practically unlimited is achievable).
    #[test]
    fn max_tool_iterations_accepts_u32_max() {
        let toml_src = r#"
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
[runtime]
max_tool_iterations = 4294967295
"#;
        let cfg: Config = toml::from_str(toml_src).expect("u32::MAX must parse");
        assert_eq!(cfg.runtime.max_tool_iterations, u32::MAX);
    }

    /// FR-03-1-2 / design doc 4.3C: `[ui] show_thinking` string parsing.
    /// Verify the 3 known values (`hidden`/`collapsed`/`expanded`) and unknown-value fallback.
    #[test]
    fn show_thinking_mode_parses_known_values() {
        for (raw, expected) in [
            ("hidden", ShowThinkingMode::Hidden),
            ("collapsed", ShowThinkingMode::Collapsed),
            ("expanded", ShowThinkingMode::Expanded),
        ] {
            let ui = UiConfig {
                show_thinking: raw.into(),
                ..UiConfig::default()
            };
            assert_eq!(ui.show_thinking_mode(), expected, "raw={raw}");
        }
    }

    #[test]
    fn show_thinking_mode_unknown_value_falls_back_to_collapsed() {
        let ui = UiConfig {
            show_thinking: "verbose".into(),
            ..UiConfig::default()
        };
        assert_eq!(ui.show_thinking_mode(), ShowThinkingMode::Collapsed);
        // Unspecified (default) also equals Collapsed.
        assert_eq!(
            UiConfig::default().show_thinking_mode(),
            ShowThinkingMode::Collapsed
        );
    }

    /// `[ui] show_progress` defaults to true, so a config file written before
    /// the progress indicator existed keeps working and shows it.
    #[test]
    fn show_progress_defaults_to_enabled() {
        assert!(UiConfig::default().show_progress);
        let cfg: Config = toml::from_str(tests_default_config()).unwrap();
        assert!(cfg.ui.show_progress, "absent key must default to true");
        let off: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[ui]
show_progress = false
"#,
        )
        .unwrap();
        assert!(!off.ui.show_progress);
    }

    /// The wheel scrollback is on by default and keeps 2000 lines, so a config
    /// file written before the keys existed behaves as the feature intends.
    #[test]
    fn the_scrollback_keys_default_to_enabled_and_two_thousand_lines() {
        assert!(UiConfig::default().mouse_scroll);
        assert_eq!(UiConfig::default().scrollback_lines, 2000);
        let cfg: Config = toml::from_str(tests_default_config()).unwrap();
        assert!(cfg.ui.mouse_scroll, "absent key must default to on");
        assert_eq!(cfg.ui.scrollback_lines, 2000);
        let off: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[ui]
mouse_scroll = false
scrollback_lines = 0
"#,
        )
        .unwrap();
        assert!(!off.ui.mouse_scroll);
        assert_eq!(
            off.ui.scrollback_lines, 0,
            "zero keeps no transcript, which also disables the wheel"
        );
        let sized: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[ui]
scrollback_lines = 500
"#,
        )
        .unwrap();
        assert_eq!(sized.ui.scrollback_lines, 500);
        assert!(sized.ui.mouse_scroll, "the other key keeps its default");
    }

    /// `[spawn]` bounds what the `spawn` tool may create; an absent section
    /// means the shipped defaults.
    #[test]
    fn the_spawn_section_defaults_to_four_children_and_depth_two() {
        let d = SpawnConfig::default();
        assert_eq!(d.max_children, 4);
        assert_eq!(d.max_depth, 2);

        let cfg: Config = toml::from_str(tests_default_config()).unwrap();
        assert_eq!(cfg.spawn.max_children, 4, "an absent section uses defaults");
        assert_eq!(cfg.spawn.max_depth, 2);

        let tuned: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[spawn]
max_children = 1
max_depth = 0
"#,
        )
        .unwrap();
        assert_eq!(tuned.spawn.max_children, 1);
        assert_eq!(tuned.spawn.max_depth, 0);

        // A partial section keeps the other default.
        let partial: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[spawn]
max_children = 8
"#,
        )
        .unwrap();
        assert_eq!(partial.spawn.max_children, 8);
        assert_eq!(partial.spawn.max_depth, 2);
    }

    /// The `[shell]` section is absent from older config files and must come
    /// up with the feature on and the same limits as `[tools.bash]`.
    #[test]
    fn the_shell_section_defaults_to_enabled_with_bash_like_limits() {
        let d = ShellConfig::default();
        assert!(d.enabled);
        assert!(d.context);
        assert_eq!(d.timeout_ms, 120_000);
        assert_eq!(d.max_output_kb, 256);

        let cfg: Config = toml::from_str(tests_default_config()).unwrap();
        assert!(cfg.shell.enabled, "an absent section must default to on");
        assert_eq!(cfg.shell.timeout_ms, 120_000);

        let tuned: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[shell]
enabled = false
timeout_ms = 5000
max_output_kb = 8
context = false
"#,
        )
        .unwrap();
        assert!(!tuned.shell.enabled);
        assert!(!tuned.shell.context);
        assert_eq!(tuned.shell.timeout_ms, 5_000);
        assert_eq!(tuned.shell.max_output_kb, 8);

        // A partial section keeps the other defaults.
        let partial: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[shell]
timeout_ms = 1000
"#,
        )
        .unwrap();
        assert_eq!(partial.shell.timeout_ms, 1_000);
        assert!(partial.shell.enabled);
        assert_eq!(partial.shell.max_output_kb, 256);
    }

    /// `[ui] color` defaults to `"auto"`, so a config file written before the
    /// colour scheme existed keeps working; unknown values fall back to it.
    #[test]
    fn color_defaults_to_auto_and_parses_known_values() {
        assert_eq!(UiConfig::default().color_mode(), ColorMode::Auto);
        let cfg: Config = toml::from_str(tests_default_config()).unwrap();
        assert_eq!(
            cfg.ui.color_mode(),
            ColorMode::Auto,
            "absent key must default to auto"
        );
        for (raw, expected) in [
            ("always", ColorMode::Always),
            ("never", ColorMode::Never),
            ("auto", ColorMode::Auto),
            ("chartreuse", ColorMode::Auto),
        ] {
            let ui = UiConfig {
                color: raw.into(),
                ..UiConfig::default()
            };
            assert_eq!(ui.color_mode(), expected, "value {raw:?}");
        }
        let never: Config = toml::from_str(
            r#"
[provider]
kind = "ollama"

[ui]
color = "never"
"#,
        )
        .unwrap();
        assert_eq!(never.ui.color_mode(), ColorMode::Never);
    }

    /// Default value of `max_tool_iterations` is 24 (raised from 8 on 2026-05-03).
    /// Omitting the `[runtime]` section also yields the same default.
    #[test]
    fn max_tool_iterations_default_is_24() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(cfg.runtime.max_tool_iterations, 24);

        let minimal = r#"
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let cfg2: Config = toml::from_str(minimal).unwrap();
        assert_eq!(cfg2.runtime.max_tool_iterations, 24);
    }

    #[test]
    fn override_provider_and_model() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        cfg.apply_overrides(Some("ollama"), Some("glm-5.1:cloud"));
        assert_eq!(cfg.provider.kind, "ollama");
        assert_eq!(
            cfg.provider.ollama.as_ref().and_then(|p| p.model.clone()),
            Some("glm-5.1:cloud".into())
        );
    }

    #[test]
    fn mask_api_key_handles_edge_cases() {
        // Empty string: returns empty as-is (caller distinguishes "not set" via Option)
        assert_eq!(mask_api_key(""), "");
        // Short key: always returns *** to avoid leaking length
        assert_eq!(mask_api_key("abc"), "***");
        assert_eq!(mask_api_key("1234567"), "***");
        // 8+ characters: first 4 + ... + last 4
        assert_eq!(mask_api_key("12345678"), "1234...5678");
        assert_eq!(mask_api_key("sk-ant-api03-XYZ-abcdef"), "sk-a...cdef");
        // Also works with typical Anthropic key length (~108 chars)
        let long: String = "sk-ant-"
            .chars()
            .chain(std::iter::repeat_n('x', 100))
            .chain("nQAA".chars())
            .collect();
        let masked = mask_api_key(&long);
        assert!(masked.starts_with("sk-a..."));
        assert!(masked.ends_with("nQAA"));
    }

    #[test]
    fn persona_overrides_apply_to_active_provider() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        cfg.apply_overrides(Some("ollama"), None);
        cfg.apply_persona_overrides(Some("custom-model"), Some(0.4));
        let entry = cfg.provider.ollama.as_ref().expect("ollama entry");
        assert_eq!(entry.model.as_deref(), Some("custom-model"));
        assert_eq!(entry.temperature, Some(0.4));
    }

    /// Documentation consistency check (T-602-10):
    /// Verify that the 3 complete sample configs from `doc/config.md` parse as `Config`.
    #[test]
    fn doc_config_md_full_samples_parse() {
        let minimal = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
"#;
        let recommended = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
thinking    = true

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"

[tools]
enabled = ["bash", "read", "write", "send_to", "list_agents", "stop_agent", "monitor", "edit", "glob", "grep", "websearch", "webfetch"]

[tools.bash]
timeout_ms    = 120000
max_output_kb = 512

[ui]
show_thinking = "collapsed"
"#;
        let full = r#"
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"
registry_dir       = "/tmp/agent-cli"
agents_dir         = "~/.config/agent-cli/agents"
persona_file       = ""

[tools]
enabled = ["bash", "read", "write", "send_to", "list_agents", "stop_agent", "monitor", "edit", "glob", "grep", "websearch", "webfetch"]

[tools.bash]
timeout_ms    = 120000
max_output_kb = 256

[ui]
show_thinking = "expanded"
"#;
        for (label, body) in [
            ("minimal", minimal),
            ("recommended", recommended),
            ("full", full),
        ] {
            let cfg: Config = toml::from_str(body)
                .unwrap_or_else(|e| panic!("doc config sample '{label}' failed to parse: {e}"));
            assert!(
                !cfg.provider.kind.is_empty(),
                "{label}: provider.kind missing"
            );
        }
    }

    /// Verify `tools.enabled` names match the implemented tools (prevent typos).
    #[test]
    fn enabled_tool_names_match_implementation() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let known = [
            "bash",
            "read",
            "write",
            "send_to",
            "spawn",
            "list_agents",
            "stop_agent",
            "monitor",
            "edit",
            "glob",
            "grep",
            "websearch",
            "webfetch",
        ];
        for name in &cfg.tools.enabled {
            assert!(
                known.contains(&name.as_str()),
                "unknown tool name in DEFAULT_CONFIG: {name}"
            );
        }
    }

    #[test]
    fn claude_code_section_parses_every_key() {
        let toml_src = r#"
[provider]
kind = "claude-code"

[provider.claude-code]
bin                = "/opt/bin/claude"
model              = "opus"
mode               = "gateway"
transport          = "oneshot"
session            = "ephemeral"
tools              = ["Bash", "Read"]
allowed_tools      = ["Bash(git *)"]
disallowed_tools   = ["WebFetch"]
permission_mode    = "acceptEdits"
turn_timeout_secs  = 120
max_budget_usd     = 0.5
system_prompt_mode = "replace"
extra_args         = ["--add-dir", "/tmp"]
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        let entry = cfg.provider.claude_code.as_ref().expect("section parsed");
        assert_eq!(entry.bin.as_deref(), Some("/opt/bin/claude"));
        assert_eq!(entry.model.as_deref(), Some("opus"));
        assert_eq!(entry.mode.as_deref(), Some("gateway"));
        assert_eq!(entry.transport.as_deref(), Some("oneshot"));
        assert_eq!(entry.session.as_deref(), Some("ephemeral"));
        assert_eq!(entry.tools.as_ref().unwrap(), &["Bash", "Read"]);
        assert_eq!(entry.allowed_tools.as_ref().unwrap()[0], "Bash(git *)");
        assert_eq!(entry.disallowed_tools.as_ref().unwrap()[0], "WebFetch");
        assert_eq!(entry.permission_mode.as_deref(), Some("acceptEdits"));
        assert_eq!(entry.turn_timeout_secs, Some(120));
        assert_eq!(entry.max_budget_usd, Some(0.5));
        assert_eq!(entry.system_prompt_mode.as_deref(), Some("replace"));
        assert_eq!(entry.extra_args.as_ref().unwrap()[1], "/tmp");
    }

    #[test]
    fn provider_entry_resolves_claude_code() {
        let toml_src = r#"
[provider]
kind = "claude-code"

[provider.claude-code]
model = "opus"
"#;
        let mut cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(
            cfg.provider_entry("claude-code")
                .and_then(|e| e.model.as_deref()),
            Some("opus")
        );
        cfg.provider_entry_mut("claude-code").unwrap().model = Some("sonnet".into());
        assert_eq!(
            cfg.provider.claude_code.as_ref().unwrap().model.as_deref(),
            Some("sonnet"),
            "the claude-code entry, not another backend's, must be mutated"
        );
    }

    #[test]
    fn config_without_claude_code_section_is_unaffected() {
        let toml_src = r#"
[provider]
kind = "ollama"

[provider.ollama]
model = "glm-5.1:cloud"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert!(cfg.provider.claude_code.is_none());
        assert!(cfg.provider_entry("claude-code").is_none());
        assert_eq!(
            cfg.provider_entry("ollama")
                .and_then(|e| e.model.as_deref()),
            Some("glm-5.1:cloud")
        );
    }

    #[test]
    fn default_config_documents_claude_code() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        // The shipped block is all comments, so the table is present but empty:
        // every key falls back to its documented default.
        let entry = cfg.provider.claude_code.as_ref().expect("section present");
        assert!(entry.bin.is_none());
        assert!(entry.mode.is_none());
        assert!(DEFAULT_CONFIG.contains("\"claude-code\""));
    }

    #[test]
    fn provider_entry_opencode_go_returns_opencode_entry() {
        let cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let entry = cfg.provider_entry("opencode-go");
        assert!(entry.is_some(), "opencode-go should resolve to opencode entry");
        assert_eq!(
            entry.unwrap().model.as_deref(),
            Some("claude-sonnet-4-5"),
            "opencode-go should share the opencode entry"
        );
    }

    #[test]
    fn provider_entry_mut_opencode_go_returns_opencode_entry() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let entry = cfg.provider_entry_mut("opencode-go");
        assert!(entry.is_some(), "opencode-go should resolve to opencode entry_mut");
        let entry = entry.unwrap();
        entry.model = Some("custom-model".to_string());
        assert_eq!(
            cfg.provider.opencode.as_ref().unwrap().model.as_deref(),
            Some("custom-model"),
            "opencode-go should mutate the opencode entry"
        );
    }

    #[test]
    fn apply_opencode_go_defaults_minimal_config() {
        let toml_src = r#"
[provider]
kind = "opencode-go"

[provider.opencode]
api_key_env = "OPENCODE_API_KEY"
"#;
        let mut cfg: Config = toml::from_str(toml_src).unwrap();
        cfg.apply_opencode_go_defaults();
        assert_eq!(cfg.provider.kind, "opencode");
        let entry = cfg.provider.opencode.as_ref().unwrap();
        assert_eq!(entry.base_url.as_deref(), Some("https://opencode.ai/zen/go/v1"));
        assert_eq!(entry.api.as_deref(), Some("openai"));
        assert_eq!(entry.model.as_deref(), Some("qwen3.8-max"));
        assert_eq!(entry.api_key_env.as_deref(), Some("OPENCODE_API_KEY"));
    }

    #[test]
    fn apply_opencode_go_defaults_all_defaults_applied() {
        let toml_src = r#"
[provider]
kind = "opencode-go"
"#;
        let mut cfg: Config = toml::from_str(toml_src).unwrap();
        cfg.apply_opencode_go_defaults();
        assert_eq!(cfg.provider.kind, "opencode");
        let entry = cfg.provider.opencode.as_ref().unwrap();
        assert_eq!(entry.base_url.as_deref(), Some("https://opencode.ai/zen/go/v1"));
        assert_eq!(entry.api.as_deref(), Some("openai"));
        assert_eq!(entry.model.as_deref(), Some("qwen3.8-max"));
        assert_eq!(entry.api_key_env.as_deref(), Some("OPENCODE_API_KEY"));
    }

    #[test]
    fn apply_opencode_go_defaults_model_is_served_by_the_go_endpoint() {
        // The Go endpoint serves open-weight models only; a `claude-*` default
        // cannot complete a turn there ("Model ... is not supported").
        let mut cfg: Config = toml::from_str("[provider]\nkind = \"opencode-go\"\n").unwrap();
        cfg.apply_opencode_go_defaults();
        let entry = cfg.provider.opencode.as_ref().unwrap();
        let model = entry.model.as_deref().unwrap();
        assert!(
            !model.starts_with("claude-"),
            "go default model must not be a claude-* id, got {model}"
        );
    }

    #[test]
    fn opencode_session_id_parses_and_defaults_to_none() {
        let toml_src = r#"
[provider]
kind = "opencode"

[provider.opencode]
session_id = "ses_pinned"
"#;
        let cfg: Config = toml::from_str(toml_src).unwrap();
        assert_eq!(
            cfg.provider.opencode.as_ref().unwrap().session_id.as_deref(),
            Some("ses_pinned")
        );
        let bare: Config = toml::from_str("[provider]\nkind = \"opencode\"\n\n[provider.opencode]\n").unwrap();
        assert!(bare.provider.opencode.as_ref().unwrap().session_id.is_none());
    }

    #[test]
    fn apply_opencode_go_defaults_preserves_explicit_values() {
        let toml_src = r#"
[provider]
kind = "opencode-go"

[provider.opencode]
api_key_env = "MY_CUSTOM_KEY"
model       = "gpt-5"
base_url    = "https://custom.example.com/v1"
api         = "openai"
"#;
        let mut cfg: Config = toml::from_str(toml_src).unwrap();
        cfg.apply_opencode_go_defaults();
        assert_eq!(cfg.provider.kind, "opencode");
        let entry = cfg.provider.opencode.as_ref().unwrap();
        assert_eq!(entry.api_key_env.as_deref(), Some("MY_CUSTOM_KEY"));
        assert_eq!(entry.model.as_deref(), Some("gpt-5"));
        assert_eq!(entry.base_url.as_deref(), Some("https://custom.example.com/v1"));
        assert_eq!(entry.api.as_deref(), Some("openai"));
    }

    #[test]
    fn apply_opencode_go_defaults_noop_for_opencode() {
        let mut cfg: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(cfg.provider.kind, "claude");
        let kind_before = cfg.provider.kind.clone();
        cfg.apply_opencode_go_defaults();
        assert_eq!(cfg.provider.kind, kind_before, "no-op for non-opencode-go kind");
    }

    // ── Per-layer alias normalization (the `duplicate field` failure) ────────
    //
    // serde rejects a table carrying both spellings of one aliased field, and
    // the layered merge matches keys by name before deserialization — so a
    // base `[tools.bash]` plus an overlay `[tools.shell]` used to fail with
    // `duplicate field `bash` in `tools``. Each layer is now normalized to the
    // canonical spellings before the merge; the tests pin the whole matrix.

    /// The provider section every test config needs.
    const PROVIDER: &str = "[provider]\nkind = \"claude\"\n";

    fn layer(tools_bash: &str, tools_shell: &str) -> String {
        let mut s = PROVIDER.to_string();
        if !tools_bash.is_empty() {
            s.push_str("\n[tools.bash]\n");
            s.push_str(tools_bash);
        }
        if !tools_shell.is_empty() {
            s.push_str("\n[tools.shell]\n");
            s.push_str(tools_shell);
        }
        s
    }

    fn parse(toml_src: &str) -> toml::Value {
        toml::from_str(toml_src).unwrap()
    }

    fn load_layers(base: &str, overlay: &str) -> Config {
        load(&ConfigSource {
            layers: vec![PathBuf::from(base), PathBuf::from(overlay)],
            from_explicit: true,
        })
        .unwrap()
    }

    #[test]
    fn the_reported_shape_loads_and_the_overlay_wins() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.toml");
        let over = dir.path().join("overlay.toml");
        std::fs::write(&base, layer("timeout_ms    = 60000", "")).unwrap();
        std::fs::write(&over, layer("", "timeout_ms    = 300000")).unwrap();
        let cfg = load_layers(base.to_str().unwrap(), over.to_str().unwrap());
        assert_eq!(cfg.tools.bash.timeout_ms, 300_000);
        assert_eq!(cfg.tools.bash.max_output_kb, 256); // the base's, kept
    }

    #[test]
    fn the_reverse_order_loads_and_the_overlay_wins() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.toml");
        let over = dir.path().join("overlay.toml");
        std::fs::write(&base, layer("", "timeout_ms    = 60000")).unwrap();
        std::fs::write(&over, layer("timeout_ms    = 300000", "")).unwrap();
        let cfg = load_layers(base.to_str().unwrap(), over.to_str().unwrap());
        assert_eq!(cfg.tools.bash.timeout_ms, 300_000);
    }

    #[test]
    fn one_file_carrying_both_spellings_loads_with_canonical_winning() {
        // The layer path is the one that matters: canonicalize the value the
        // way `load` does, then deserialize the result.
        let (v, renames) = canonicalize_aliases(
            parse(&layer("timeout_ms    = 60000", "timeout_ms    = 300000")),
            Path::new("x.toml"),
        );
        assert_eq!(renames.len(), 1);
        let cfg: Config = v.try_into().unwrap();
        // The canonical spelling ([tools.bash]) wins the conflict; the legacy
        // table only fills keys the canonical one does not set.
        assert_eq!(cfg.tools.bash.timeout_ms, 60_000);
        // The legacy table's keys fill where the canonical one is absent:
        let (v, _) = canonicalize_aliases(
            parse(&layer(
                "timeout_ms    = 60000",
                "max_output_kb = 512",
            )),
            Path::new("x.toml"),
        );
        let fills: Config = v.try_into().unwrap();
        assert_eq!(fills.tools.bash.timeout_ms, 60_000);
        assert_eq!(fills.tools.bash.max_output_kb, 512);
    }

    #[test]
    fn the_sibling_alias_pair_loads_in_both_orders() {
        let both = |base: &str, over: &str| -> String {
            let dir = tempfile::tempdir().unwrap();
            let (b, o) = (dir.path().join("base.toml"), dir.path().join("overlay.toml"));
            std::fs::write(&b, base).unwrap();
            std::fs::write(&o, over).unwrap();
            load(&ConfigSource {
                layers: vec![b, o],
                from_explicit: true,
            })
            .unwrap()
            .permissions
            .default_mode
            .clone()
            .unwrap()
        };
        assert_eq!(
            both(
                "[provider]\nkind = \"claude\"\n[permissions]\ndefaultMode = \"deny\"\n",
                "[provider]\nkind = \"claude\"\n[permissions]\ndefault_mode = \"allow\"\n"
            ),
            "allow"
        );
        assert_eq!(
            both(
                "[provider]\nkind = \"claude\"\n[permissions]\ndefault_mode = \"allow\"\n",
                "[provider]\nkind = \"claude\"\n[permissions]\ndefaultMode = \"deny\"\n"
            ),
            "deny"
        );
        assert_eq!(
            both(
                "[provider]\nkind = \"claude\"\n[permissions]\ndefaultMode = \"deny\"\n",
                "[provider]\nkind = \"claude\"\n[permissions]\ndefault_mode = \"ask\"\n"
            ),
            "ask"
        );
    }

    #[test]
    fn the_controls_load_exactly_as_before() {
        // Alias-only:
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.toml");
        let over = dir.path().join("overlay.toml");
        std::fs::write(&base, layer("", "timeout_ms    = 60000")).unwrap();
        let cfg = load(&ConfigSource {
            layers: vec![base.clone()],
            from_explicit: true,
        })
        .unwrap();
        assert_eq!(cfg.tools.bash.timeout_ms, 60_000);
        // Same spelling in both layers — the overlay wins (today's rule):
        std::fs::write(&over, layer("timeout_ms    = 300000", "")).unwrap();
        let cfg = load(&ConfigSource {
            layers: vec![base, over],
            from_explicit: true,
        })
        .unwrap();
        assert_eq!(cfg.tools.bash.timeout_ms, 300_000);
    }

    #[test]
    fn renames_are_reported_naming_the_file_and_the_pair() {
        let dir = tempfile::tempdir().unwrap();
        let over = dir.path().join("overlay.toml");
        std::fs::write(&over, layer("", "timeout_ms    = 60000")).unwrap();
        let value: toml::Value = toml::from_str(&std::fs::read_to_string(&over).unwrap()).unwrap();
        let (_, renames) = canonicalize_aliases(value, &over);
        assert_eq!(renames.len(), 1);
        let r = &renames[0];
        assert!(r.contains("[tools.shell] → [tools.bash]"), "{r}");
        assert!(r.contains(over.to_str().unwrap()), "{r}");
    }

    #[test]
    fn a_still_broken_merged_configuration_names_the_chain() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.toml");
        let over = dir.path().join("overlay.toml");
        std::fs::write(&base, PROVIDER).unwrap();
        std::fs::write(&over, PROVIDER).unwrap(); // provider twice: no conflict
        // A genuine type error the normalization does not touch:
        std::fs::write(&over, "[provider]\nkind = \"claude\"\n\n[ui]\nmouse_scroll = \"not-a-bool\"\n").unwrap();
        let err = load(&ConfigSource {
            layers: vec![base, over],
            from_explicit: true,
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("merged configuration ("), "{err}");
        assert!(err.contains("base.toml"), "{err}");
        assert!(err.contains("overlay.toml"), "{err}");
    }
}
