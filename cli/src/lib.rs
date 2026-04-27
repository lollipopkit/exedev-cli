mod cli;
mod cli_command;
mod output;
mod ssh;

use anyhow::{Context, Result};
use clap::Parser;
use exedev_core::{API_KEY_ENV, client::ExeDevClient, shell};
use std::env;

pub async fn run() -> Result<()> {
    exedev_core::env::load_dotenv()?;
    let cli = cli::Cli::parse();
    run_cli(cli).await
}

async fn run_cli(cli: cli::Cli) -> Result<()> {
    let built = cli_command::build_command(&cli.command)?;
    let command_string = shell::shell_join(&built.words);

    if command_string == "exit" {
        println!("exit is only meaningful in the ssh exe.dev REPL; exedev-ctl is exiting.");
        return Ok(());
    }

    shell::guard_dangerous_command(&command_string, cli.yes)?;

    if built.fallback_ssh {
        return ssh::run_ssh_fallback(&built.words, cli.json).await;
    }

    let api_key = env::var(API_KEY_ENV)
        .with_context(|| format!("missing {API_KEY_ENV}; export an exe.dev HTTPS API key first"))?;
    let response = ExeDevClient::new(cli.endpoint, api_key)
        .exec(&command_string)
        .await?;
    output::print_response(&response, cli.json)?;
    Ok(())
}
