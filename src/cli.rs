use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "agent-cli",
    version,
    about = "Standalone multi-agent CLI with Claude Code-equivalent functionality",
    long_about = None
)]
pub struct Cli {
    /// Config file path to use. When unspecified, resolves in order: AGENT_CLI_CONFIG -> ~/.config/agent-cli/config.toml.
    #[arg(long, global = true, env = "AGENT_CLI_CONFIG")]
    pub config: Option<PathBuf>,

    /// REPL startup options (available even when subcommand is omitted).
    #[command(flatten)]
    pub run_args: RunArgs,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start REPL and begin conversation as one agent
    Run,

    /// Run headless: register and serve peers over IPC without an interactive
    /// REPL. This is the target a detached `spawn` launches; it can also be run
    /// directly to get a foreground headless agent.
    Serve,

    /// Spawn a detached agent-cli process that does not depend on this one:
    /// it runs in its own session, self-registers as a peer, and outlives the
    /// launcher. Stop it later with `stop`.
    Spawn,

    /// Stop a running peer by sending a graceful shutdown request (falls back
    /// to SIGTERM by pid on transport failure).
    Stop {
        /// Destination agent-id or display name
        peer: String,
    },

    /// List running peers
    List {
        /// Only list agents in this group
        #[arg(long)]
        group: Option<String>,
    },

    /// Detect and list the groups currently running as processes
    Groups,

    /// Send a prompt to the specified peer
    Send {
        /// Destination agent-id or display name
        peer: String,
        /// Prompt text to send
        text: String,
    },

    /// Send a prompt to a running peer and print the response
    Ask {
        /// Destination agent-id or display name
        peer: String,
        /// Prompt text to send
        text: String,
        /// Reply timeout in seconds (default: 120)
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },

    /// Show available backends and configuration status
    Providers,

    /// Check configuration, API keys, backend connectivity, registry, and shell tools
    Doctor,

    /// Rebuild and replace agent-cli from `main` (a source build via `cargo`,
    /// like the installer). Use `--ref` to build another branch or a tag, or
    /// `--check` to only report whether a newer release exists.
    Update {
        /// Only report the running version against the latest release; make no changes
        #[arg(long)]
        check: bool,
        /// Skip the confirmation prompt and reinstall unconditionally
        #[arg(long)]
        force: bool,
        /// Do not prompt for confirmation before replacing the binary
        #[arg(long)]
        yes: bool,
        /// Tag (vX.Y.Z) or branch to build from [default: main]
        #[arg(long = "ref")]
        git_ref: Option<String>,
    },

    /// Smoke test with a short prompt and tool execution
    Selftest {
        /// Backend to verify (defaults to config.provider.kind when unspecified)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Configuration operations
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Inspect configured MCP (Model Context Protocol) servers
    Mcp {
        #[command(subcommand)]
        action: McpAction,
    },
}

#[derive(Parser, Debug, Default, Clone)]
pub struct RunArgs {
    /// Agent display name
    #[arg(long, global = true)]
    pub name: Option<String>,

    /// Group id this agent joins; detached children inherit it. Overrides the
    /// `[runtime] group` config key.
    #[arg(long, global = true)]
    pub group: Option<String>,

    /// AI backend (claude / claude-code / codex / ollama / opencode / opencode-go / llama.cpp)
    #[arg(long, global = true)]
    pub provider: Option<String>,

    /// Override the backend model name
    #[arg(long, global = true)]
    pub model: Option<String>,

    /// Path to the agent persona file
    #[arg(long, global = true)]
    pub persona: Option<PathBuf>,

    /// Auto-approve tool execution without confirmation
    #[arg(long, global = true)]
    pub auto_approve_tools: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Open configuration file in editor
    Edit,
    /// Show resolved configuration file path
    Path,
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// Connect to the configured MCP servers and list their tools
    List,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_help_compiles_and_lists_known_subcommands() {
        let cmd = Cli::command();
        let names: Vec<String> = cmd
            .get_subcommands()
            .map(|sc| sc.get_name().to_string())
            .collect();
        for required in &[
            "run",
            "serve",
            "spawn",
            "stop",
            "list",
            "groups",
            "send",
            "providers",
            "doctor",
            "update",
            "selftest",
            "config",
            "ask",
            "mcp",
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "subcommand `{required}` missing (have: {names:?})"
            );
        }
    }

    #[test]
    fn cli_parses_run_with_persona_and_provider() {
        // Explicit run subcommand
        let cli = Cli::try_parse_from([
            "agent-cli",
            "--config",
            "/tmp/x.toml",
            "run",
            "--name",
            "alice",
            "--provider",
            "ollama",
            "--model",
            "glm-5.1:cloud",
            "--persona",
            "/tmp/p.md",
            "--auto-approve-tools",
        ])
        .expect("parse run args");
        assert!(cli.config.is_some());
        assert!(matches!(cli.command, Some(Command::Run)));
        assert_eq!(cli.run_args.name.as_deref(), Some("alice"));
        assert_eq!(cli.run_args.provider.as_deref(), Some("ollama"));
        assert_eq!(cli.run_args.model.as_deref(), Some("glm-5.1:cloud"));
        assert!(cli.run_args.auto_approve_tools);

        // Subcommand omitted (FR-01 equivalence)
        let cli = Cli::try_parse_from([
            "agent-cli",
            "--name",
            "alice",
            "--provider",
            "ollama",
            "--model",
            "glm-5.1:cloud",
            "--persona",
            "/tmp/p.md",
            "--auto-approve-tools",
        ])
        .expect("parse run args without subcommand");
        assert!(cli.command.is_none());
        assert_eq!(cli.run_args.name.as_deref(), Some("alice"));
        assert_eq!(cli.run_args.provider.as_deref(), Some("ollama"));
        assert_eq!(cli.run_args.model.as_deref(), Some("glm-5.1:cloud"));
        assert!(cli.run_args.persona.is_some());
        assert!(cli.run_args.auto_approve_tools);
    }

    #[test]
    fn cli_parses_send_subcommand() {
        let cli =
            Cli::try_parse_from(["agent-cli", "send", "alice", "hello world"]).expect("parse send");
        match cli.command {
            Some(Command::Send { peer, text }) => {
                assert_eq!(peer, "alice");
                assert_eq!(text, "hello world");
            }
            other => panic!("expected Send, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_config_subcommands() {
        for action in ["show", "edit", "path"] {
            let cli = Cli::try_parse_from(["agent-cli", "config", action])
                .unwrap_or_else(|e| panic!("parse config {action}: {e}"));
            match cli.command {
                Some(Command::Config { .. }) => {}
                other => panic!("expected Config, got {other:?}"),
            }
        }
    }

    #[test]
    fn cli_parses_ask_subcommand() {
        let cli = Cli::try_parse_from(["agent-cli", "ask", "alice", "hello"])
            .expect("parse ask");
        match cli.command {
            Some(Command::Ask { peer, text, timeout }) => {
                assert_eq!(peer, "alice");
                assert_eq!(text, "hello");
                assert_eq!(timeout, 120);
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_spawn_with_run_args() {
        let cli = Cli::try_parse_from(["agent-cli", "spawn", "--name", "worker", "--provider", "ollama"])
            .expect("parse spawn");
        assert!(matches!(cli.command, Some(Command::Spawn)));
        assert_eq!(cli.run_args.name.as_deref(), Some("worker"));
        assert_eq!(cli.run_args.provider.as_deref(), Some("ollama"));
    }

    #[test]
    fn cli_parses_serve() {
        let cli = Cli::try_parse_from(["agent-cli", "serve", "--name", "worker"]).expect("parse serve");
        assert!(matches!(cli.command, Some(Command::Serve)));
        assert_eq!(cli.run_args.name.as_deref(), Some("worker"));
    }

    #[test]
    fn cli_parses_group_flag() {
        let cli = Cli::try_parse_from(["agent-cli", "spawn", "--group", "team", "--name", "w"])
            .expect("parse spawn --group");
        assert!(matches!(cli.command, Some(Command::Spawn)));
        assert_eq!(cli.run_args.group.as_deref(), Some("team"));

        let cli = Cli::try_parse_from(["agent-cli", "serve", "--group", "team"])
            .expect("parse serve --group");
        assert!(matches!(cli.command, Some(Command::Serve)));
        assert_eq!(cli.run_args.group.as_deref(), Some("team"));
    }

    #[test]
    fn cli_parses_list_group() {
        let cli = Cli::try_parse_from(["agent-cli", "list", "--group", "team"])
            .expect("parse list --group");
        match cli.command {
            Some(Command::List { group }) => assert_eq!(group.as_deref(), Some("team")),
            other => panic!("expected List, got {other:?}"),
        }

        // No filter → None
        let cli = Cli::try_parse_from(["agent-cli", "list"]).expect("parse list");
        match cli.command {
            Some(Command::List { group }) => assert!(group.is_none()),
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_groups() {
        let cli = Cli::try_parse_from(["agent-cli", "groups"]).expect("parse groups");
        assert!(matches!(cli.command, Some(Command::Groups)));
    }

    #[test]
    fn cli_parses_update() {
        let cli = Cli::try_parse_from(["agent-cli", "update"]).expect("parse update");
        match cli.command {
            Some(Command::Update { check, force, yes, git_ref }) => {
                assert!(!check && !force && !yes && git_ref.is_none());
            }
            other => panic!("expected Update, got {other:?}"),
        }

        let cli = Cli::try_parse_from([
            "agent-cli", "update", "--check", "--force", "--yes", "--ref", "v0.6.0",
        ])
        .expect("parse update with flags");
        match cli.command {
            Some(Command::Update { check, force, yes, git_ref }) => {
                assert!(check && force && yes);
                assert_eq!(git_ref.as_deref(), Some("v0.6.0"));
            }
            other => panic!("expected Update, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_mcp() {
        let cli = Cli::try_parse_from(["agent-cli", "mcp", "list"]).expect("parse mcp list");
        match cli.command {
            Some(Command::Mcp { action }) => {
                assert!(matches!(action, crate::cli::McpAction::List));
            }
            other => panic!("expected Mcp, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_stop() {
        let cli = Cli::try_parse_from(["agent-cli", "stop", "worker"]).expect("parse stop");
        match cli.command {
            Some(Command::Stop { peer }) => assert_eq!(peer, "worker"),
            other => panic!("expected Stop, got {other:?}"),
        }
    }

    #[test]
    fn cli_parses_ask_with_timeout() {
        let cli = Cli::try_parse_from(["agent-cli", "ask", "--timeout", "30", "alice", "hi"])
            .expect("parse ask with timeout");
        match cli.command {
            Some(Command::Ask { peer, text, timeout }) => {
                assert_eq!(peer, "alice");
                assert_eq!(text, "hi");
                assert_eq!(timeout, 30);
            }
            other => panic!("expected Ask, got {other:?}"),
        }
    }
}
