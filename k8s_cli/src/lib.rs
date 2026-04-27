mod cli;
mod fleet;
mod manager;

use anyhow::Result;
use clap::Parser;

pub async fn run() -> Result<()> {
    let cli = cli::K8sCli::parse();
    manager::run(cli).await
}
