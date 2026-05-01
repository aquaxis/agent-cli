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
    /// 使用する設定ファイルパス。未指定時は AGENT_CLI_CONFIG → ~/.config/agent-cli/config.toml の順で解決。
    #[arg(long, global = true, env = "AGENT_CLI_CONFIG")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// REPL を起動して 1 エージェントとして対話を開始
    Run(RunArgs),

    /// 稼働中のピア一覧を表示
    List,

    /// 指定ピアにプロンプトを送信
    Send {
        /// 宛先 agent-id または表示名
        peer: String,
        /// 送信するプロンプト本文
        text: String,
    },

    /// 利用可能なバックエンドと設定状況を表示
    Providers,

    /// 設定・APIキー・バックエンド疎通・レジストリ・シェルツールを点検
    Doctor,

    /// 短いプロンプトとツール実行で動作確認するスモークテスト
    Selftest {
        /// 検証対象バックエンド（未指定時は config.provider.kind）
        #[arg(long)]
        provider: Option<String>,
    },

    /// 設定操作
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Parser, Debug, Default, Clone)]
pub struct RunArgs {
    /// エージェントの表示名
    #[arg(long)]
    pub name: Option<String>,

    /// AI バックエンド (claude / codex / ollama / llama.cpp)
    #[arg(long)]
    pub provider: Option<String>,

    /// バックエンドのモデル名を上書き
    #[arg(long)]
    pub model: Option<String>,

    /// エージェントペルソナファイルのパス
    #[arg(long)]
    pub persona: Option<PathBuf>,

    /// ツール実行を確認なしで自動承認
    #[arg(long)]
    pub auto_approve_tools: bool,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// 現在の設定を表示
    Show,
    /// 設定ファイルをエディタで開く
    Edit,
    /// 解決済みの設定ファイルパスを表示
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
        match cli.command {
            Some(Command::Run(args)) => {
                assert_eq!(args.name.as_deref(), Some("alice"));
                assert_eq!(args.provider.as_deref(), Some("ollama"));
                assert_eq!(args.model.as_deref(), Some("glm-5.1:cloud"));
                assert!(args.auto_approve_tools);
            }
            other => panic!("expected Run, got {other:?}"),
        }
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
