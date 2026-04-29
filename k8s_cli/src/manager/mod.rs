use super::{
    cli::{
        BootstrapCmd, ClusterMode, DeployCmd, DestroyCmd, K8sCli, K8sCommands, PlanCmd, StatusCmd,
    },
    fleet::{FleetFile, FleetPlan, NodeRole, NodeSpec},
};
use crate::output;
use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use exedev_core::{
    API_KEY_ENV,
    client::{ExeDevApiError, ExeDevClient},
    shell,
};
use std::{
    collections::BTreeSet,
    env,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use tokio::time::{Duration, sleep};

mod api_proxy;
mod kubectl;
mod parsing;
mod process;
mod scripts;
mod state;
#[cfg(test)]
mod tests;

use kubectl::{
    KUBECTL_PROBE_REQUEST_TIMEOUT, kubectl_apply, kubectl_capture, kubectl_capture_with_timeout,
    kubectl_run_owned,
};
use parsing::{parse_kubernetes_nodes, parse_vm_names};
use process::{
    ensure_tool, remote_capture, run_remote_interactive_script, upload_script_dir, verify_vm_access,
};
use scripts::{BOOTSTRAP_NODE_SCRIPT, BootstrapNodeRequest, BootstrapNodeRole, script_dir};
use state::{
    generated_kubeconfig_path, generated_token_path, read_or_create_k3s_token, write_secret_file,
};

const TS_AUTHKEY_ENV: &str = "TS_AUTHKEY";
const K3S_TOKEN_ENV: &str = "K3S_TOKEN";
const K3S_URL_ENV: &str = "K3S_URL";
const KUBERNETES_API_WAIT_ATTEMPTS: usize = 24;
const KUBERNETES_NODE_WAIT_ATTEMPTS: usize = 30;
const KUBERNETES_WAIT_DELAY: Duration = Duration::from_secs(5);
const LOCAL_K8S_API_CONNECT_TIMEOUT: StdDuration = StdDuration::from_secs(3);
pub(crate) async fn run(cli: K8sCli) -> Result<()> {
    match cli.command {
        K8sCommands::Plan(cmd) => run_plan(&cli.endpoint, cmd).await,
        K8sCommands::Bootstrap(cmd) => run_bootstrap(&cli.endpoint, cli.yes, cmd).await,
        K8sCommands::Deploy(cmd) => run_deploy(cmd).await,
        K8sCommands::Status(cmd) => run_status(&cli.endpoint, cmd).await,
        K8sCommands::Destroy(cmd) => run_destroy(&cli.endpoint, cmd).await,
    }
}

async fn run_plan(endpoint: &str, cmd: PlanCmd) -> Result<()> {
    let plan = load_plan(&cmd.fleet)?;
    let current = fetch_current_vms(endpoint).await?;
    print_bootstrap_plan(&plan, cmd.mode, &current, None);
    Ok(())
}

async fn run_bootstrap(endpoint: &str, yes: bool, cmd: BootstrapCmd) -> Result<()> {
    let plan = load_plan(&cmd.fleet)?;
    ensure_tool("ssh").await?;
    ensure_tool("scp").await?;
    ensure_tool("kubectl").await?;
    let ts_authkey = require_env(TS_AUTHKEY_ENV)?;
    if cmd.mode == ClusterMode::Existing {
        require_env(K3S_URL_ENV)?;
        require_env(K3S_TOKEN_ENV)?;
    }
    let include_control_plane = cmd.mode == ClusterMode::New;
    let current = fetch_current_vms(endpoint).await?;
    print_bootstrap_plan(&plan, cmd.mode, &current, cmd.manifests.as_deref());
    confirm("Run this bootstrap plan?", yes)?;

    let api_key = exe_api_key()?;
    let client = ExeDevClient::new(endpoint.to_string(), api_key.clone());
    create_missing_vms(&client, &plan, include_control_plane, &current, &cmd.fleet).await?;
    let new_cluster_access = bootstrap_k3s(
        &plan,
        cmd.mode,
        &ts_authkey,
        cmd.kubeconfig.as_deref(),
        endpoint,
        &api_key,
        yes,
    )
    .await?;

    let kubeconfig = kubeconfig_for_bootstrap(&plan, cmd.mode, cmd.kubeconfig.as_deref());
    wait_for_kubernetes_api(kubeconfig.as_deref(), new_cluster_access.as_ref()).await?;
    wait_for_kubernetes_nodes(&plan, include_control_plane, kubeconfig.as_deref()).await?;
    apply_node_metadata(&plan, include_control_plane, kubeconfig.as_deref()).await?;
    if let Some(manifests) = cmd.manifests {
        kubectl_apply(kubeconfig.as_deref(), &manifests).await?;
    }
    Ok(())
}

async fn run_deploy(cmd: DeployCmd) -> Result<()> {
    ensure_tool("kubectl").await?;
    kubectl_apply(cmd.kubeconfig.as_deref(), &cmd.manifests).await
}

async fn run_status(endpoint: &str, cmd: StatusCmd) -> Result<()> {
    let plan = load_plan(&cmd.fleet)?;
    ensure_tool("kubectl").await?;
    let current = fetch_current_vms(endpoint).await?;
    print_vm_status(&plan, &current);
    print_kubernetes_status(&plan, cmd.kubeconfig.as_deref()).await?;
    Ok(())
}

async fn run_destroy(endpoint: &str, cmd: DestroyCmd) -> Result<()> {
    let plan = load_plan(&cmd.fleet)?;
    let current = fetch_current_vms(endpoint).await?;
    let managed = if cmd.all_planned {
        plan.nodes.iter().collect::<Vec<_>>()
    } else {
        plan.nodes
            .iter()
            .filter(|node| current.contains(&node.name))
            .collect::<Vec<_>>()
    };
    if managed.is_empty() {
        println!("{}", output::muted("No fleet-managed VMs found."));
        return Ok(());
    }

    println!("{}", output::heading("Destroy will delete these VMs:"));
    for node in &managed {
        println!("  - {}", output::vm(&node.name));
    }
    confirm("Delete these exe.dev VMs?", false)?;
    let client = exe_client(endpoint)?;
    for node in managed {
        let command = shell::shell_join(&["rm".into(), node.name.clone()]);
        println!("{} {command}", output::label("exe.dev:"));
        client.exec(&command).await?;
    }
    Ok(())
}

fn load_plan(path: &Path) -> Result<FleetPlan> {
    Ok(FleetFile::load(path)?.to_plan())
}

fn exe_client(endpoint: &str) -> Result<ExeDevClient> {
    Ok(ExeDevClient::new(endpoint.to_string(), exe_api_key()?))
}

fn exe_api_key() -> Result<String> {
    env::var(API_KEY_ENV)
        .with_context(|| format!("missing {API_KEY_ENV}; export an exe.dev HTTPS API key first"))
}

async fn fetch_current_vms(endpoint: &str) -> Result<BTreeSet<String>> {
    let client = exe_client(endpoint)?;
    let response = client.exec("ls").await?;
    parse_vm_names(&response).context("failed to parse exe.dev ls response")
}

fn print_bootstrap_plan(
    plan: &FleetPlan,
    mode: ClusterMode,
    current: &BTreeSet<String>,
    manifests: Option<&Path>,
) {
    let include_control_plane = mode == ClusterMode::New;
    println!(
        "{} {} {}",
        output::heading("Cluster:"),
        output::label(&plan.cluster_name),
        output::muted(format!("({}, {})", plan.network, plan.kubernetes))
    );
    println!(
        "{} {}",
        output::heading("Mode:"),
        output::label(mode_name(mode))
    );
    println!("{}", output::heading("VMs to create:"));
    let mut any = false;
    for node in plan.bootstrap_nodes(include_control_plane) {
        if !current.contains(&node.name) {
            any = true;
            println!(
                "  - {} [{}] image={}",
                output::vm(&node.name),
                output::role(role_name(node.role)),
                output::label(&node.image)
            );
        }
    }
    if !any {
        println!("  {}", output::muted("none"));
    }

    println!("{}", output::heading("Bootstrap nodes:"));
    for node in plan.bootstrap_nodes(include_control_plane) {
        println!(
            "  - {} [{}] pool={}",
            output::vm(&node.name),
            output::role(role_name(node.role)),
            output::label(&node.pool)
        );
    }

    println!("{}", output::heading("Kubernetes metadata:"));
    for node in plan.bootstrap_nodes(include_control_plane) {
        let labels = node
            .labels
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        let taint = node.taint.as_deref().unwrap_or("none");
        println!(
            "  - {} labels=[{}] taint={}",
            output::vm(&node.name),
            output::label(labels),
            output::label(taint)
        );
    }

    if let Some(path) = manifests {
        println!(
            "{} {}",
            output::heading("Deploy manifests:"),
            output::label(path.display())
        );
    }
}

fn print_vm_status(plan: &FleetPlan, current: &BTreeSet<String>) {
    println!("{}", output::heading("exe.dev VMs:"));
    for node in &plan.nodes {
        let state = if current.contains(&node.name) {
            output::success("present")
        } else {
            output::warn("missing")
        };
        println!(
            "  - {} [{}] {}",
            output::vm(&node.name),
            output::role(role_name(node.role)),
            state
        );
    }
}

async fn print_kubernetes_status(plan: &FleetPlan, kubeconfig: Option<&Path>) -> Result<()> {
    let output = kubectl_capture(kubeconfig, &["get", "nodes", "-o", "json"]).await?;
    let nodes = parse_kubernetes_nodes(&output)?;
    println!("{}", output::heading("Kubernetes nodes:"));
    for expected in &plan.nodes {
        if let Some(actual) = nodes.get(&expected.name) {
            let labels_ok = expected
                .labels
                .iter()
                .all(|(key, value)| actual.labels.get(key) == Some(value));
            let taint_ok = match &expected.taint {
                Some(taint) => actual.taints.contains(taint),
                None => true,
            };
            println!(
                "  - {} ready={} labels={} taint={}",
                output::vm(&expected.name),
                ok_text(actual.ready),
                ok_text(labels_ok),
                ok_text(taint_ok)
            );
        } else {
            println!(
                "  - {} {}",
                output::vm(&expected.name),
                output::warn("missing")
            );
        }
    }
    Ok(())
}

async fn create_missing_vms(
    client: &ExeDevClient,
    plan: &FleetPlan,
    include_control_plane: bool,
    current: &BTreeSet<String>,
    fleet_path: &Path,
) -> Result<()> {
    for node in plan.bootstrap_nodes(include_control_plane) {
        if current.contains(&node.name) {
            continue;
        }
        let command = exe_new_command(node);
        println!("{} {command}", output::label("exe.dev:"));
        if let Err(err) = client.exec(&command).await {
            if is_vm_name_unavailable_error(&err, &node.name) {
                println!(
                    "{} VM name {} is not available; verifying SSH access before continuing",
                    output::warn("exe.dev:"),
                    output::vm(&node.name)
                );
                verify_vm_access(&node.name, fleet_path).await?;
                println!(
                    "{} verified SSH access to {}; continuing",
                    output::success("exe.dev:"),
                    output::vm(&node.name)
                );
                continue;
            }
            return Err(err);
        }
    }
    Ok(())
}

async fn bootstrap_k3s(
    plan: &FleetPlan,
    mode: ClusterMode,
    ts_authkey: &str,
    kubeconfig_arg: Option<&Path>,
    endpoint: &str,
    api_key: &str,
    yes: bool,
) -> Result<Option<NewClusterAccess>> {
    let api_proxy = api_proxy::ApiProxy::start(endpoint.to_string(), api_key.to_string())?;
    match mode {
        ClusterMode::New => {
            let controls = plan.control_planes();
            let control = *controls
                .first()
                .context("fleet has no control-plane node")?;
            let mut token = read_or_create_k3s_token(&plan.cluster_name)?;
            bootstrap_node(
                plan,
                control,
                BootstrapNodeRole::ControlPlanePrimary,
                ts_authkey,
                &token,
                None,
                yes,
                &api_proxy,
            )
            .await?;
            let control_ip = fetch_tailscale_ip(&control.name).await?;
            let control_ip_addr = control_ip.parse::<Ipv4Addr>().with_context(|| {
                format!("invalid Tailscale IPv4 for {}: {control_ip}", control.name)
            })?;
            let k3s_url = format!("https://{control_ip}:6443");
            token = fetch_k3s_node_token(&control.name).await?;
            write_secret_file(&generated_token_path(&plan.cluster_name), &token)?;
            let kubeconfig = fetch_kubeconfig(&control.name, &control_ip).await?;
            let kubeconfig_path = kubeconfig_arg
                .map(Path::to_path_buf)
                .unwrap_or_else(|| generated_kubeconfig_path(&plan.cluster_name));
            write_secret_file(&kubeconfig_path, &kubeconfig)?;
            println!(
                "{} {}",
                output::success("Wrote kubeconfig:"),
                output::label(kubeconfig_path.display())
            );

            for control in controls.iter().copied().skip(1) {
                bootstrap_node(
                    plan,
                    control,
                    BootstrapNodeRole::ControlPlaneJoin,
                    ts_authkey,
                    &token,
                    Some(&k3s_url),
                    yes,
                    &api_proxy,
                )
                .await?;
            }

            for node in plan
                .nodes
                .iter()
                .filter(|node| node.role != NodeRole::ControlPlane)
            {
                bootstrap_node(
                    plan,
                    node,
                    BootstrapNodeRole::Worker,
                    ts_authkey,
                    &token,
                    Some(&k3s_url),
                    yes,
                    &api_proxy,
                )
                .await?;
            }
            Ok(Some(NewClusterAccess {
                control_name: control.name.clone(),
                control_ip: control_ip_addr,
            }))
        }
        ClusterMode::Existing => {
            let k3s_url = require_env(K3S_URL_ENV)?;
            let token = require_env(K3S_TOKEN_ENV)?;
            if kubeconfig_arg.is_none() && env::var_os("KUBECONFIG").is_none() {
                println!(
                    "{} no --kubeconfig or KUBECONFIG set; kubectl will use its default config",
                    output::warn("warning:")
                );
            }
            for node in plan
                .nodes
                .iter()
                .filter(|node| node.role != NodeRole::ControlPlane)
            {
                bootstrap_node(
                    plan,
                    node,
                    BootstrapNodeRole::Worker,
                    ts_authkey,
                    &token,
                    Some(&k3s_url),
                    yes,
                    &api_proxy,
                )
                .await?;
            }
            Ok(None)
        }
    }
}

async fn bootstrap_node(
    plan: &FleetPlan,
    node: &NodeSpec,
    role: BootstrapNodeRole,
    ts_authkey: &str,
    k3s_token: &str,
    k3s_server_url: Option<&str>,
    assume_yes: bool,
    api_proxy: &api_proxy::ApiProxy,
) -> Result<()> {
    let local_scripts = script_dir();
    let remote_dir = remote_script_dir(&plan.cluster_name, &node.name);
    upload_script_dir(&node.name, &local_scripts, &remote_dir).await?;
    let request = BootstrapNodeRequest {
        vm_name: node.name.clone(),
        role,
        cluster_name: plan.cluster_name.clone(),
        ts_authkey: ts_authkey.into(),
        k3s_token: k3s_token.into(),
        k3s_server_url: k3s_server_url.map(str::to_string),
        assume_yes,
    };
    run_remote_interactive_script(
        &node.name,
        &remote_dir,
        BOOTSTRAP_NODE_SCRIPT,
        &request.envs(),
        Some(api_proxy),
    )
    .await
}

fn remote_script_dir(cluster_name: &str, node_name: &str) -> String {
    format!(
        "/tmp/exedev-k8s-{}-{}",
        safe_remote_name(cluster_name),
        safe_remote_name(node_name)
    )
}

fn safe_remote_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn remote_privileged_script(command: &str) -> String {
    format!(
        r#"
if [ "$(id -u)" -eq 0 ]; then
  SUDO=""
elif command -v sudo >/dev/null 2>&1; then
  SUDO="sudo"
else
  echo "root or sudo is required" >&2
  exit 127
fi
{command}
"#
    )
}

async fn fetch_tailscale_ip(vm: &str) -> Result<String> {
    let ip = remote_capture(vm, "tailscale ip -4 | head -n1").await?;
    let ip = ip.trim();
    if ip.is_empty() {
        bail!("failed to detect Tailscale IPv4 for {vm}");
    }
    ip.parse::<Ipv4Addr>()
        .with_context(|| format!("invalid Tailscale IPv4 for {vm}: {ip}"))?;
    Ok(ip.to_string())
}

async fn fetch_kubeconfig(vm: &str, control_ip: &str) -> Result<String> {
    let script = remote_privileged_script("${SUDO} cat /etc/rancher/k3s/k3s.yaml");
    let kubeconfig = remote_capture(vm, &script).await?;
    Ok(kubeconfig
        .replace(
            "https://127.0.0.1:6443",
            &format!("https://{control_ip}:6443"),
        )
        .replace(
            "https://localhost:6443",
            &format!("https://{control_ip}:6443"),
        ))
}

async fn fetch_k3s_node_token(vm: &str) -> Result<String> {
    let script = remote_privileged_script("${SUDO} cat /var/lib/rancher/k3s/server/node-token");
    remote_capture(vm, &script)
        .await
        .map(|token| token.trim().to_string())
        .with_context(|| format!("failed to fetch k3s node token from {vm}"))
}

async fn wait_for_kubernetes_api(
    kubeconfig: Option<&Path>,
    new_cluster_access: Option<&NewClusterAccess>,
) -> Result<()> {
    println!(
        "{}",
        output::heading("Waiting for Kubernetes API to become ready...")
    );
    let mut last_error = String::new();
    for attempt in 1..=KUBERNETES_API_WAIT_ATTEMPTS {
        match kubectl_capture_with_timeout(
            kubeconfig,
            &["get", "--raw=/readyz"],
            KUBECTL_PROBE_REQUEST_TIMEOUT,
        )
        .await
        {
            Ok(output) if output.trim() == "ok" || output.contains("[+]") => return Ok(()),
            Ok(output) => last_error = output.trim().to_string(),
            Err(err) => last_error = err.to_string(),
        }
        if attempt < KUBERNETES_API_WAIT_ATTEMPTS {
            println!(
                "{} Kubernetes API is not ready yet; retrying ({attempt}/{KUBERNETES_API_WAIT_ATTEMPTS})",
                output::warn("waiting:")
            );
            sleep(KUBERNETES_WAIT_DELAY).await;
        }
    }
    if let Some(access) = new_cluster_access {
        let local_detail = local_kubernetes_api_detail(access.control_ip);
        let remote_detail = diagnose_control_plane(&access.control_name)
            .await
            .unwrap_or_else(|err| format!("failed to collect remote diagnostics: {err}"));
        bail!(
            "Kubernetes API did not become ready: {last_error}\n{local_detail}\n{}\nRemote control-plane diagnostics from {}:\n{remote_detail}",
            tailscale_policy_hint(),
            access.control_name
        );
    }
    bail!("Kubernetes API did not become ready: {last_error}");
}

fn local_kubernetes_api_detail(control_ip: Ipv4Addr) -> String {
    let address = SocketAddr::V4(SocketAddrV4::new(control_ip, 6443));
    match TcpStream::connect_timeout(&address, LOCAL_K8S_API_CONNECT_TIMEOUT) {
        Ok(_) => format!("local TCP check: connected to {address}"),
        Err(err) => format!(
            "local TCP check: cannot connect to {address}: {err}. Check that this machine is logged in to the same Tailscale tailnet and can reach the control-plane VM. This may also be a Tailscale ACL/Grants permissions issue."
        ),
    }
}

fn tailscale_policy_hint() -> &'static str {
    "Tailscale policy hint: ensure workers can reach the control-plane on tcp:6443, for example tag:server -> tag:server tcp:6443, and ensure your local kubectl client can reach the control-plane on tcp:6443."
}

async fn diagnose_control_plane(control_name: &str) -> Result<String> {
    let script = remote_privileged_script(
        r#"
echo "k3s readyz from control-plane:"
${SUDO} k3s kubectl get --raw=/readyz 2>&1 || true
echo
echo "k3s service state:"
if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
  ${SUDO} systemctl is-active k3s 2>&1 || true
  ${SUDO} systemctl status k3s --no-pager -l 2>&1 | tail -n 40 || true
fi
echo
echo "listeners on 6443:"
if command -v ss >/dev/null 2>&1; then
  ${SUDO} ss -ltnp 2>&1 | grep ':6443' || true
elif command -v netstat >/dev/null 2>&1; then
  ${SUDO} netstat -ltnp 2>&1 | grep ':6443' || true
else
  echo "ss/netstat not available"
fi
echo
echo "tailscale self:"
tailscale ip -4 2>&1 || true
tailscale status --self 2>&1 || true
"#,
    );
    remote_capture(control_name, &script).await
}

async fn wait_for_kubernetes_nodes(
    plan: &FleetPlan,
    include_control_plane: bool,
    kubeconfig: Option<&Path>,
) -> Result<()> {
    let expected = plan
        .bootstrap_nodes(include_control_plane)
        .into_iter()
        .map(|node| node.name.clone())
        .collect::<BTreeSet<_>>();
    println!(
        "{}",
        output::heading("Waiting for Kubernetes nodes to register...")
    );
    let mut last_error = String::new();
    for attempt in 1..=KUBERNETES_NODE_WAIT_ATTEMPTS {
        match kubectl_capture_with_timeout(
            kubeconfig,
            &["get", "nodes", "-o", "json"],
            KUBECTL_PROBE_REQUEST_TIMEOUT,
        )
        .await
        {
            Ok(output) => {
                let nodes = parse_kubernetes_nodes(&output)?;
                let missing = expected
                    .iter()
                    .filter(|name| !nodes.contains_key(*name))
                    .cloned()
                    .collect::<Vec<_>>();
                if missing.is_empty() {
                    return Ok(());
                }
                last_error = format!("missing nodes: {}", missing.join(", "));
            }
            Err(err) => last_error = err.to_string(),
        }
        if attempt < KUBERNETES_NODE_WAIT_ATTEMPTS {
            println!(
                "{} Kubernetes nodes are not ready yet ({last_error}); retrying ({attempt}/{KUBERNETES_NODE_WAIT_ATTEMPTS})",
                output::warn("waiting:")
            );
            sleep(KUBERNETES_WAIT_DELAY).await;
        }
    }
    bail!("Kubernetes nodes did not register: {last_error}");
}

async fn apply_node_metadata(
    plan: &FleetPlan,
    include_control_plane: bool,
    kubeconfig: Option<&Path>,
) -> Result<()> {
    for node in plan.bootstrap_nodes(include_control_plane) {
        let mut label_args = vec!["label".into(), "node".into(), node.name.clone()];
        label_args.extend(
            node.labels
                .iter()
                .map(|(key, value)| format!("{key}={value}")),
        );
        label_args.push("--overwrite".into());
        kubectl_run_owned(kubeconfig, label_args).await?;

        if let Some(taint) = &node.taint {
            kubectl_run_owned(
                kubeconfig,
                vec![
                    "taint".into(),
                    "node".into(),
                    node.name.clone(),
                    taint.clone(),
                    "--overwrite".into(),
                ],
            )
            .await?;
        }
    }
    Ok(())
}

fn kubeconfig_for_bootstrap(
    plan: &FleetPlan,
    mode: ClusterMode,
    kubeconfig: Option<&Path>,
) -> Option<PathBuf> {
    kubeconfig.map(Path::to_path_buf).or_else(|| {
        (mode == ClusterMode::New).then(|| generated_kubeconfig_path(&plan.cluster_name))
    })
}

fn exe_new_command(node: &NodeSpec) -> String {
    shell::shell_join(&[
        "new".into(),
        "--name".into(),
        node.name.clone(),
        "--image".into(),
        node.image.clone(),
        "--no-email".into(),
    ])
}

fn is_vm_name_unavailable_error(err: &anyhow::Error, vm_name: &str) -> bool {
    err.downcast_ref::<ExeDevApiError>().is_some_and(|api| {
        api.status().as_u16() == 422 && is_vm_name_unavailable_body(api.body(), vm_name)
    })
}

fn is_vm_name_unavailable_body(body: &str, vm_name: &str) -> bool {
    body.contains("not available") && body.contains(vm_name)
}

#[derive(Debug, Eq, PartialEq)]
struct NewClusterAccess {
    control_name: String,
    control_ip: Ipv4Addr,
}

fn confirm(prompt: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    let proceed = Confirm::new()
        .with_prompt(prompt)
        .default(false)
        .interact()
        .context("failed to read confirmation")?;
    if !proceed {
        bail!("operation cancelled");
    }
    Ok(())
}

fn require_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing {name}"))
}

fn mode_name(mode: ClusterMode) -> &'static str {
    match mode {
        ClusterMode::New => "new",
        ClusterMode::Existing => "existing",
    }
}

fn role_name(role: NodeRole) -> &'static str {
    match role {
        NodeRole::ControlPlane => "control-plane",
        NodeRole::Worker => "worker",
        NodeRole::Spare => "spare",
    }
}

fn ok_text(ok: bool) -> String {
    if ok {
        output::success("ok")
    } else {
        output::warn("drift")
    }
}
