use crate::output;
use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use exedev_core::shell;
use std::{collections::BTreeMap, path::Path, process::Stdio};
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::time::{Duration, sleep, timeout};

const REMOTE_EXIT_PREFIX: &str = "__EXEDEV_K8S_EXIT__:";

const REMOTE_SSH_ATTEMPTS: usize = 5;

const REMOTE_SSH_RETRY_DELAY: Duration = Duration::from_secs(3);

/// Upper bound on a single remote step, covering the whole exchange rather than
/// just the connect that `ConnectTimeout` bounds. Generous enough that the slowest
/// real step (a k3s or Tailscale install on a cold VM) never reaches it, so hitting
/// it means the remote side is stuck rather than slow.
const REMOTE_SSH_TIMEOUT: Duration = Duration::from_secs(900);

/// Upper bound on a captured local command. Every caller is a kubectl read whose
/// own `--request-timeout` is at most 30s, so this only fires when kubectl itself
/// is stuck rather than waiting on the API.
const CAPTURE_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

const TAILNET_LOCK_AUTH_REQUIRED_STATUS: i32 = 126;

/// Emitted by `CHECK_TAILNET_LOCK_SCRIPT` alongside its 126 exit.
const TAILNET_LOCK_MARKER: &str = "Tailnet Lock is enabled and this VM is locked out";

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

/// SSH destinations for fleet VMs, keyed by VM name.
#[derive(Debug, Default)]
pub(super) struct SshTargets(BTreeMap<String, String>);

impl SshTargets {
    pub(super) fn new(destinations: BTreeMap<String, String>) -> Self {
        Self(destinations)
    }

    /// The destination reported by exe.dev, or the `<vm>.exe.xyz` hostname when
    /// exe.dev did not report one (for example a VM outside this account's `ls`).
    pub(super) fn dest(&self, vm: &str) -> String {
        self.0
            .get(vm)
            .cloned()
            .unwrap_or_else(|| format!("{vm}.exe.xyz"))
    }
}

pub(super) async fn remote_run(targets: &SshTargets, vm: &str, script: &str) -> Result<()> {
    loop {
        let output = remote_command_output(targets, vm, script).await?;
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
        // 126 is also the conventional shell status for "found but not executable",
        // so the status alone does not identify the Tailnet Lock case. Pairing it
        // with the message the check emits keeps an unrelated 126 reported as the
        // failure it is, rather than prompting for a signature and then rerunning
        // a step that already changed state.
        if output.status != TAILNET_LOCK_AUTH_REQUIRED_STATUS
            || !output.stderr.contains(TAILNET_LOCK_MARKER)
        {
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

pub(super) async fn remote_capture(targets: &SshTargets, vm: &str, script: &str) -> Result<String> {
    let output = remote_command_output(targets, vm, script).await?;
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

pub(super) async fn remote_command_output(
    targets: &SshTargets,
    vm: &str,
    script: &str,
) -> Result<RemoteCommandOutput> {
    let wrapped_script = remote_status_script(vm, script);
    let args = remote_ssh_args(&targets.dest(vm));
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = capture_remote_ssh_output(&refs, &wrapped_script).await?;
    parse_remote_command_output(vm, output)
}

pub(super) async fn verify_vm_access(
    targets: &SshTargets,
    vm: &str,
    fleet_path: &Path,
) -> Result<()> {
    remote_run(targets, vm, "true").await.with_context(|| {
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

pub(super) async fn capture_command(program: &str, args: &[&str]) -> Result<String> {
    Ok(capture_command_output(program, args).await?.stdout)
}

pub(super) async fn capture_command_output(program: &str, args: &[&str]) -> Result<CommandOutput> {
    println!(
        "{}",
        output::command(format!("$ {}", display_command(program, args)))
    );
    let child = TokioCommand::new(program)
        .args(args)
        .stdin(Stdio::null())
        // Dropped by the timeout below, which must take the process with it.
        .kill_on_drop(true)
        .output();
    // `--request-timeout` bounds kubectl's API call, not kubectl itself: a
    // kubeconfig exec plugin, a credential helper, or a wedged resolver can hang
    // before any request is made, which would otherwise consume the whole polling
    // window in one attempt and never reach the diagnostics.
    let output = match timeout(CAPTURE_COMMAND_TIMEOUT, child).await {
        Ok(result) => result.with_context(|| format!("failed to run {program}"))?,
        Err(_) => bail!(
            "{program} produced no result within {}s and was killed",
            CAPTURE_COMMAND_TIMEOUT.as_secs()
        ),
    };
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
            // The timed future below is dropped when it elapses, which must take the
            // ssh process and its pipes with it rather than leaking both.
            .kill_on_drop(true)
            .spawn()
            .context("failed to run ssh")?;
        // Both the script write and the wait are inside the timeout: a remote side
        // that stops reading stdin blocks the write just as a hung script blocks
        // the wait.
        let attempt_result = timeout(REMOTE_SSH_TIMEOUT, async {
            let write_result = if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(script.as_bytes())
                    .await
                    .map_err(anyhow::Error::from)
            } else {
                Ok(())
            };
            child
                .wait_with_output()
                .await
                .context("failed to wait for ssh")
                .map(|output| (write_result, output))
        })
        .await;
        let (write_result, output) = match attempt_result {
            Ok(result) => result?,
            // Not retried: a step that stops responding is not the transient
            // transport failure the 255 retry below exists for, and rerunning it
            // would repeat whatever the remote side already did.
            Err(_) => bail!(
                "remote command on this VM produced no result within {}s and was killed; check the VM directly, then rerun exedev-k8s bootstrap",
                REMOTE_SSH_TIMEOUT.as_secs()
            ),
        };
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
        // The wrapper prints the exit marker once the remote script has finished.
        // Seeing it means ssh failed while returning output, not before running
        // anything, so resending the script would repeat an install or a service
        // change that already happened.
        let remote_ran = String::from_utf8_lossy(&output.stdout).contains(REMOTE_EXIT_PREFIX);
        if output.status.code() == Some(255) && !remote_ran && attempt < REMOTE_SSH_ATTEMPTS {
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

pub(super) fn remote_ssh_args(dest: &str) -> Vec<String> {
    vec![
        "-o".into(),
        "ControlMaster=no".into(),
        "-o".into(),
        "ControlPath=none".into(),
        "-o".into(),
        "StrictHostKeyChecking=accept-new".into(),
        "-o".into(),
        "ConnectTimeout=15".into(),
        // The destination comes from the exe.dev API, and ssh parses options up to
        // the first non-option word, so without this a reported destination
        // starting with `-` would be read as a local ssh option such as
        // `-oProxyCommand=...` instead of a host.
        "--".into(),
        dest.to_string(),
        "sh".into(),
        "-s".into(),
    ]
}

pub(super) fn redact_command_secrets(command: &str) -> String {
    let mut redacted = redact_prefixed_secret(command, "tskey-auth-", "tskey-auth-<redacted>");
    for key in ["K3S_BOOTSTRAP_TOKEN=", "K3S_TOKEN="] {
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
