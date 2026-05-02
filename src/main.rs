use clap::Parser;
use tracing_subscriber::EnvFilter;

mod agent;
mod ai;
mod app;
mod cli;
mod commands;
mod config;
mod error;
mod id;
mod ipc;
mod log;
mod persona;
mod tools;

use crate::cli::{Cli, Command, ConfigAction};
use crate::error::Result;

#[tokio::main]
async fn main() {
    init_tracing();
    // FR-13「アプリ終了」：run() が Ok を返した時点で全 Drop は実行済みのため、
    // tokio runtime の停止を待たずに即座にプロセス終了する。
    // （`tokio::io::stdin()` の内部ブロッキングスレッドが残ると runtime drop で
    // EOF まで待たされ、SIGINT/SIGTERM 経路で終了が遅延するため。）
    match run().await {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,agent_cli=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let source = config::resolve_path(cli.config.as_deref())?;

    match cli.command.unwrap_or(Command::Run) {
        Command::Run => {
            let cfg = config::load(&source)?;
            app::run(cfg, cli.run_args).await
        }
        Command::List => {
            let cfg = config::load(&source)?;
            commands::list(&cfg).await
        }
        Command::Send { peer, text } => {
            let cfg = config::load(&source)?;
            commands::send(&cfg, &peer, &text).await
        }
        Command::Providers => {
            let cfg = config::load(&source)?;
            commands::providers(&cfg).await
        }
        Command::Doctor => {
            let cfg = config::load(&source)?;
            commands::doctor(&cfg, &source).await
        }
        Command::Selftest { provider } => {
            let cfg = config::load(&source)?;
            commands::selftest(&cfg, provider.as_deref()).await
        }
        Command::Config { action } => match action {
            ConfigAction::Show => {
                let cfg = config::load(&source)?;
                commands::config_show(&cfg)
            }
            ConfigAction::Edit => commands::config_edit(&source),
            ConfigAction::Path => {
                println!("{}", source.path.display());
                Ok(())
            }
        },
    }
}
