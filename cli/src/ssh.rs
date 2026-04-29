use anyhow::{Context, Result, bail};
use exedev_core::{shell, terminal};
use std::process::Stdio;
use tokio::process::Command as TokioCommand;

pub(crate) async fn run_ssh_fallback(words: &[String], json: bool) -> Result<()> {
    let mut display_words = vec!["ssh".to_string(), "exe.dev".to_string()];
    display_words.extend(words.iter().cloned());
    if json {
        display_words.push("--json".to_string());
    }
    println!("{}", terminal::command(shell::shell_join(&display_words)));

    let mut command = TokioCommand::new("ssh");
    command.arg("exe.dev");
    command.args(words);
    if json {
        command.arg("--json");
    }
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());
    let status = command
        .status()
        .await
        .context("failed to run local ssh exe.dev")?;
    if !status.success() {
        bail!("ssh exe.dev exited with status {status}");
    }
    Ok(())
}
