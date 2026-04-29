use super::api_proxy::ApiProxy;
use crate::output;
use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use exedev_core::shell;
use std::{path::Path, process::Stdio};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::time::{Duration, sleep};

const REMOTE_EXIT_PREFIX: &str = "__EXEDEV_K8S_EXIT__:";

const REMOTE_SSH_ATTEMPTS: usize = 5;

const REMOTE_SSH_RETRY_DELAY: Duration = Duration::from_secs(3);

const TAILNET_LOCK_AUTH_REQUIRED_STATUS: i32 = 126;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CommandOutput {
    stdout: String,
    stderr: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct RemoteCommandOutput {
    stdout: String,
    stderr: String,
    status: i32,
}

pub(super) async fn remote_run(vm: &str, script: &str) -> Result<()> {
    loop {
        let output = remote_command_output(vm, script).await?;
        if !output.stdout.is_empty() {
            print!("{}", output.stdout);
            if !output.stdout.ends_with('\n') {
                println!();
            }
        }
        if !output.stderr.is_empty() {
            eprint!("{}", output::stderr_block(&output.stderr));
            if !output.stderr.ends_with('\n') {
                eprintln!();
            }
        }
        if output.status == 0 {
            return Ok(());
        }
        if output.status != TAILNET_LOCK_AUTH_REQUIRED_STATUS {
            bail!(
                "remote command on {vm} exited with status {}",
                output.status
            );
        }
        if !confirm_tailnet_lock_retry(vm)? {
            bail!(
                "bootstrap paused: Tailnet Lock authorization is required; sign the VM on a trusted signing node, then rerun exedev-k8s bootstrap"
            );
        }
    }
}

fn confirm_tailnet_lock_retry(vm: &str) -> Result<bool> {
    println!(
        "{} Tailnet Lock/ACL is blocking {}; sign the node or update ACLs, then confirm to retry this step.",
        output::warn("paused:"),
        output::vm(vm)
    );
    Confirm::new()
        .with_prompt("I have signed the node or updated ACLs")
        .default(false)
        .interact()
        .context("failed to read Tailnet Lock confirmation")
}

pub(super) async fn remote_capture(vm: &str, script: &str) -> Result<String> {
    let output = remote_command_output(vm, script).await?;
    if output.status != 0 {
        let detail = [output.stdout.trim(), output.stderr.trim()]
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        if detail.is_empty() {
            bail!(
                "remote command on {vm} exited with status {}",
                output.status
            );
        }
        bail!(
            "remote command on {vm} exited with status {}: {detail}",
            output.status
        );
    }
    Ok(output.stdout)
}

pub(super) async fn remote_command_output(vm: &str, script: &str) -> Result<RemoteCommandOutput> {
    let wrapped_script = remote_status_script(vm, script);
    let args = remote_ssh_args(vm);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = capture_remote_ssh_output(&refs, &wrapped_script).await?;
    parse_remote_command_output(vm, output)
}

pub(super) async fn verify_vm_access(vm: &str, fleet_path: &Path) -> Result<()> {
    remote_run(vm, "true").await.with_context(|| {
        format!(
            "VM name {vm} is unavailable but SSH access could not be verified; recover with `exedev-k8s destroy --fleet {} --all-planned`, or choose another vmPrefix",
            fleet_path.display()
        )
    })
}

pub(super) async fn ensure_tool(tool: &str) -> Result<()> {
    let status = TokioCommand::new("sh")
        .arg("-c")
        .arg(format!("command -v {tool} >/dev/null 2>&1"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .with_context(|| format!("failed to check for tool `{tool}`"))?;
    if !status.success() {
        bail!("required tool `{tool}` was not found in PATH");
    }
    Ok(())
}

pub(super) async fn upload_script_dir(vm: &str, script_dir: &Path, remote_dir: &str) -> Result<()> {
    if !script_dir.is_dir() {
        bail!("script directory does not exist: {}", script_dir.display());
    }
    remote_run(
        vm,
        &format!(
            "rm -rf {remote_dir} && mkdir -p {remote_dir}",
            remote_dir = shell::shell_join(&[remote_dir.to_string()])
        ),
    )
    .await?;

    let source = format!("{}/.", script_dir.display());
    let target = format!("{vm}.exe.xyz:{remote_dir}");
    let args = scp_args(&source, &target);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_command("scp", &refs, Stdio::inherit()).await
}

pub(super) async fn run_remote_interactive_script(
    vm: &str,
    remote_dir: &str,
    script_name: &str,
    envs: &[(String, String)],
    api_proxy: Option<&ApiProxy>,
) -> Result<()> {
    let mut args = remote_ssh_base_args(vm);
    args.insert(0, "-tt".into());
    if let Some(proxy) = api_proxy {
        args.splice(
            0..0,
            [
                "-o".into(),
                "ExitOnForwardFailure=yes".into(),
                "-R".into(),
                format!("127.0.0.1:{port}:127.0.0.1:{port}", port = proxy.port()),
            ],
        );
    }
    args.push(remote_interactive_command(
        remote_dir,
        script_name,
        envs,
        api_proxy,
    ));
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_interactive_command("ssh", &refs).await
}

pub(super) async fn run_command(program: &str, args: &[&str], stdout: Stdio) -> Result<()> {
    println!(
        "{}",
        output::command(format!("$ {}", display_command(program, args)))
    );
    let status = TokioCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} exited with status {status}");
    }
    Ok(())
}

pub(super) async fn run_interactive_command(program: &str, args: &[&str]) -> Result<()> {
    println!(
        "{}",
        output::command(format!("$ {}", display_command(program, args)))
    );
    let status = TokioCommand::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
        .with_context(|| format!("failed to run {program}"))?;
    if !status.success() {
        bail!("{program} exited with status {status}");
    }
    Ok(())
}

pub(super) async fn capture_command(program: &str, args: &[&str]) -> Result<String> {
    Ok(capture_command_output(program, args).await?.stdout)
}

pub(super) async fn capture_command_output(program: &str, args: &[&str]) -> Result<CommandOutput> {
    println!(
        "{}",
        output::command(format!("$ {}", display_command(program, args)))
    );
    let output = TokioCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        bail!(
            "{program} exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub(super) async fn capture_remote_ssh_output(
    args: &[&str],
    script: &str,
) -> Result<CommandOutput> {
    let mut last_status = None;
    let mut last_detail = String::new();
    for attempt in 1..=REMOTE_SSH_ATTEMPTS {
        println!(
            "{}",
            output::command(format!(
                "$ {} <remote-script>",
                display_command("ssh", args)
            ))
        );
        let mut child = TokioCommand::new("ssh")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to run ssh")?;
        let write_result = if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(script.as_bytes())
                .await
                .map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        let output = child
            .wait_with_output()
            .await
            .context("failed to wait for ssh")?;
        if let Err(err) = write_result
            && output.status.success()
        {
            return Err(err).context("failed to send remote script to ssh");
        }
        if output.status.success() {
            return Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        last_status = Some(output.status);
        last_detail = command_output_detail(&output.stdout, &output.stderr);
        if output.status.code() == Some(255) && attempt < REMOTE_SSH_ATTEMPTS {
            eprintln!(
                "{}",
                output::stderr_block(format!(
                    "ssh exited with status 255 before the remote command completed; retrying ({attempt}/{REMOTE_SSH_ATTEMPTS})"
                ))
            );
            sleep(REMOTE_SSH_RETRY_DELAY).await;
            continue;
        }

        break;
    }

    let status = last_status.context("ssh did not run")?;
    if last_detail.is_empty() {
        bail!("ssh exited with status {status}");
    }
    bail!("ssh exited with status {status}: {last_detail}");
}

pub(super) fn command_output_detail(stdout: &[u8], stderr: &[u8]) -> String {
    [
        String::from_utf8_lossy(stderr).trim().to_string(),
        String::from_utf8_lossy(stdout).trim().to_string(),
    ]
    .into_iter()
    .filter(|text| !text.is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

pub(super) fn parse_remote_command_output(
    vm: &str,
    output: CommandOutput,
) -> Result<RemoteCommandOutput> {
    let (stdout, status) = parse_remote_stdout(vm, &output.stdout)?;
    Ok(RemoteCommandOutput {
        stdout,
        stderr: output.stderr,
        status,
    })
}

pub(super) fn parse_remote_stdout(vm: &str, stdout: &str) -> Result<(String, i32)> {
    let marker_start = stdout
        .rfind(REMOTE_EXIT_PREFIX)
        .with_context(|| format!("remote command on {vm} did not report an exit status"))?;
    let command_stdout = stdout[..marker_start].trim_end_matches('\n').to_string();
    let status_text = stdout[marker_start + REMOTE_EXIT_PREFIX.len()..]
        .lines()
        .next()
        .unwrap_or("")
        .trim();
    let status = status_text.parse::<i32>().with_context(|| {
        format!("remote command on {vm} reported invalid exit status `{status_text}`")
    })?;
    Ok((command_stdout, status))
}

pub(super) fn remote_status_script(expected_hostname: &str, script: &str) -> String {
    let expected = shell::shell_join(&[expected_hostname.to_string()]);
    format!(
        "__exedev_k8s_expected_hostname={expected}\n__exedev_k8s_actual_hostname=\"$(hostname -s 2>/dev/null || hostname 2>/dev/null || true)\"\nif [ \"$__exedev_k8s_actual_hostname\" != \"$__exedev_k8s_expected_hostname\" ]; then\n  echo \"exedev-k8s target mismatch: expected $__exedev_k8s_expected_hostname, reached $__exedev_k8s_actual_hostname\" >&2\n  __exedev_k8s_status=125\nelse\n  (\n{script}\n  )\n  __exedev_k8s_status=$?\nfi\nprintf '\\n{REMOTE_EXIT_PREFIX}%s\\n' \"$__exedev_k8s_status\"\nexit 0"
    )
}

pub(super) fn display_command(program: &str, args: &[&str]) -> String {
    let words = std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();
    redact_command_secrets(&shell::shell_join(&words))
}

pub(super) fn remote_ssh_args(vm: &str) -> Vec<String> {
    let mut args = remote_ssh_base_args(vm);
    args.extend(["sh".into(), "-s".into()]);
    args
}

pub(super) fn remote_ssh_base_args(vm: &str) -> Vec<String> {
    vec![
        "-o".into(),
        "ControlMaster=no".into(),
        "-o".into(),
        "ControlPath=none".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        format!("{vm}.exe.xyz"),
    ]
}

pub(super) fn scp_args(source: &str, target: &str) -> Vec<String> {
    vec![
        "-r".into(),
        "-o".into(),
        "ControlMaster=no".into(),
        "-o".into(),
        "ControlPath=none".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        source.into(),
        target.into(),
    ]
}

pub(super) fn remote_interactive_command(
    remote_dir: &str,
    script_name: &str,
    envs: &[(String, String)],
    api_proxy: Option<&ApiProxy>,
) -> String {
    let cd = shell::shell_join(&["cd".into(), remote_dir.into()]);
    let mut words = vec!["env".to_string()];
    words.extend(envs.iter().map(|(key, value)| format!("{key}={value}")));
    if let Some(proxy) = api_proxy {
        words.push(format!("EXEDEV_CLI_API_URL={}", proxy.url()));
        words.push(format!("EXEDEV_CLI_API_TOKEN={}", proxy.token()));
    }
    words.extend(["bash".into(), format!("./{script_name}")]);
    format!("{cd} && {}", shell::shell_join(&words))
}

pub(super) fn redact_command_secrets(command: &str) -> String {
    let mut redacted = redact_prefixed_secret(command, "tskey-auth-", "tskey-auth-<redacted>");
    for key in [
        "K3S_BOOTSTRAP_TOKEN=",
        "K3S_TOKEN=",
        "EXEDEV_CLI_API_TOKEN=",
    ] {
        redacted = redact_assignment_value(&redacted, key);
    }
    redacted
}

pub(super) fn redact_prefixed_secret(command: &str, prefix: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(start) = rest.find(prefix) {
        output.push_str(&rest[..start]);
        output.push_str(replacement);
        let after_prefix = start + prefix.len();
        let end = rest[after_prefix..]
            .find(is_secret_delimiter)
            .map(|offset| after_prefix + offset)
            .unwrap_or(rest.len());
        rest = &rest[end..];
    }
    output.push_str(rest);
    output
}

pub(super) fn redact_assignment_value(command: &str, key: &str) -> String {
    let mut output = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(start) = rest.find(key) {
        output.push_str(&rest[..start]);
        let after_key = start + key.len();
        let value = &rest[after_key..];
        if value.starts_with('$') || value.starts_with("\"$") || value.starts_with("'$") {
            output.push_str(key);
            rest = value;
            continue;
        }
        output.push_str(key);
        output.push_str("<redacted>");
        let end = rest[after_key..]
            .find(is_assignment_delimiter)
            .map(|offset| after_key + offset)
            .unwrap_or(rest.len());
        rest = &rest[end..];
    }
    output.push_str(rest);
    output
}

pub(super) fn is_secret_delimiter(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '\'' | '"' | '\\')
}

pub(super) fn is_assignment_delimiter(ch: char) -> bool {
    ch == '\n' || ch == '\r'
}
