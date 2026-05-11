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

    /// List running peers
    List,

    /// Send a prompt to the specified peer
    Send {
        /// Destination agent-id or display name
        peer: String,
        /// Prompt text to send
        text: String,
    },

    /// Show available backends and configuration status
    Providers,

    /// Check configuration, API keys, backend connectivity, registry, and shell tools
    Doctor,

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
}

#[derive(Parser, Debug, Default, Clone)]
pub struct RunArgs {
    /// Agent display name
    #[arg(long, global = true)]
    pub name: Option<String>,

    /// AI backend (claude / codex / ollama / llama.cpp)
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
            "list",
            "send",
            "providers",
            "doctor",
            "selftest",
            "config",
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
}
