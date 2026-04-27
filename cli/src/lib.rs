mod cli;
mod client;
mod command;
mod output;
mod ssh;

use anyhow::{Context, Result};
use clap::Parser;
use std::env;

const DEFAULT_ENDPOINT: &str = "https://exe.dev/exec";
const API_KEY_ENV: &str = "EXE_DEV_API_KEY";

pub async fn run() -> Result<()> {
    let cli = cli::Cli::parse();
    run_cli(cli).await
}

async fn run_cli(cli: cli::Cli) -> Result<()> {
    let built = command::build_command(&cli.command)?;
    let command_string = command::shell_join(&built.words);

    if command_string == "exit" {
        println!("exit is only meaningful in the ssh exe.dev REPL; exedevctl is exiting.");
        return Ok(());
    }

    command::guard_dangerous_command(&command_string, cli.yes)?;

    if built.fallback_ssh {
        return ssh::run_ssh_fallback(&built.words, cli.json).await;
    }

    let api_key = env::var(API_KEY_ENV)
        .with_context(|| format!("missing {API_KEY_ENV}; export an exe.dev HTTPS API key first"))?;
    let response = client::ExeDevClient::new(cli.endpoint, api_key)
        .exec(&command_string)
        .await?;
    output::print_response(&response, cli.json)?;
    Ok(())
}
