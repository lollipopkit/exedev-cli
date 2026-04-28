mod cli;
mod fleet;
mod manager;
mod output;

use anyhow::Result;
use clap::Parser;

pub async fn run() -> Result<()> {
    exedev_core::env::load_dotenv()?;
    let cli = cli::K8sCli::parse();
    manager::run(cli).await
}

pub fn format_error(message: impl std::fmt::Display) -> String {
    output::error(message)
}
