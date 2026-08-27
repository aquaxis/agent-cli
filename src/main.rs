use clap::Parser;
use tracing_subscriber::EnvFilter;

mod agent;
mod ai;
mod app;
mod cli;
mod commands;
mod config;
mod custom_commands;
mod error;
mod editor;
mod history;
mod id;
mod ipc;
mod log;
mod persona;
mod tools;
mod update;

use crate::cli::{Cli, Command, ConfigAction};
use crate::error::Result;

#[tokio::main]
async fn main() {
    init_tracing();
    // FR-13 "App termination": When run() returns Ok, all Drops have already executed,
    // so we exit immediately without waiting for the tokio runtime to shut down.
    // (If the `tokio::io::stdin()` internal blocking thread remains, runtime drop
    // waits until EOF, delaying termination via SIGINT/SIGTERM.)
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
            app::run(cfg, source, cli.run_args).await
        }
        Command::Serve => {
            let cfg = config::load(&source)?;
            app::run_headless(cfg, source, cli.run_args).await
        }
        Command::Spawn => {
            let cfg = config::load(&source)?;
            commands::spawn(&cfg, &source, cli.run_args).await
        }
        Command::Stop { peer } => {
            let cfg = config::load(&source)?;
            commands::stop(&cfg, &peer).await
        }
        Command::List { group } => {
            let cfg = config::load(&source)?;
            commands::list(&cfg, group.as_deref()).await
        }
        Command::Groups => {
            let cfg = config::load(&source)?;
            commands::groups(&cfg).await
        }
        Command::Send { peer, text } => {
            let cfg = config::load(&source)?;
            commands::send(&cfg, &peer, &text).await
        }
        Command::Ask { peer, text, timeout } => {
            let cfg = config::load(&source)?;
            commands::ask(&cfg, &peer, &text, timeout).await
        }
        Command::Providers => {
            let cfg = config::load(&source)?;
            commands::providers(&cfg).await
        }
        Command::Doctor => {
            let mut cfg = config::load(&source)?;
            commands::doctor(&mut cfg, &source).await
        }
        // `update` depends only on compile-time identity + the network, not on
        // the config file, so it is dispatched without `config::load`.
        Command::Update { check, force, yes, git_ref } => {
            update::run(update::UpdateOpts { check, force, yes, git_ref }).await
        }
        Command::Selftest { provider } => {
            let cfg = config::load(&source)?;
            commands::selftest(&cfg, &source, provider.as_deref()).await
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
