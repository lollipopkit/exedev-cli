use super::{
    cli::{
        BootstrapCmd, ClusterMode, DeployCmd, DestroyCmd, K8sCli, K8sCommands, PlanCmd, StatusCmd,
    },
    fleet::{FleetFile, FleetPlan, NodeRole, NodeSpec},
};
use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use exedev_core::{API_KEY_ENV, client::ExeDevClient, shell};
use rand::{Rng, distr::Alphanumeric};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command as TokioCommand;

const STATE_DIR: &str = ".exedev-k8s";
const TS_AUTHKEY_ENV: &str = "TS_AUTHKEY";
const K3S_TOKEN_ENV: &str = "K3S_TOKEN";
const K3S_URL_ENV: &str = "K3S_URL";

pub(crate) async fn run(cli: K8sCli) -> Result<()> {
    match cli.command {
        K8sCommands::Plan(cmd) => run_plan(&cli.endpoint, cmd).await,
        K8sCommands::Bootstrap(cmd) => run_bootstrap(&cli.endpoint, cli.yes, cmd).await,
        K8sCommands::Deploy(cmd) => run_deploy(cmd).await,
        K8sCommands::Status(cmd) => run_status(&cli.endpoint, cmd).await,
        K8sCommands::Destroy(cmd) => run_destroy(&cli.endpoint, cli.yes, cmd).await,
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

    let client = exe_client(endpoint)?;
    create_missing_vms(&client, &plan, include_control_plane, &current).await?;
    bootstrap_k3s(&plan, cmd.mode, &ts_authkey, cmd.kubeconfig.as_deref()).await?;

    let kubeconfig = kubeconfig_for_bootstrap(&plan, cmd.mode, cmd.kubeconfig.as_deref());
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

async fn run_destroy(endpoint: &str, yes: bool, cmd: DestroyCmd) -> Result<()> {
    let plan = load_plan(&cmd.fleet)?;
    let current = fetch_current_vms(endpoint).await?;
    let managed = plan
        .nodes
        .iter()
        .filter(|node| current.contains(&node.name))
        .collect::<Vec<_>>();
    if managed.is_empty() {
        println!("No fleet-managed VMs found.");
        return Ok(());
    }

    println!("Destroy will delete these VMs:");
    for node in &managed {
        println!("  - {}", node.name);
    }
    confirm("Delete these exe.dev VMs?", yes)?;
    let client = exe_client(endpoint)?;
    for node in managed {
        let command = shell::shell_join(&["rm".into(), node.name.clone()]);
        println!("exe.dev: {command}");
        client.exec(&command).await?;
    }
    Ok(())
}

fn load_plan(path: &Path) -> Result<FleetPlan> {
    Ok(FleetFile::load(path)?.to_plan())
}

fn exe_client(endpoint: &str) -> Result<ExeDevClient> {
    let api_key = env::var(API_KEY_ENV)
        .with_context(|| format!("missing {API_KEY_ENV}; export an exe.dev HTTPS API key first"))?;
    Ok(ExeDevClient::new(endpoint.to_string(), api_key))
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
        "Cluster: {} ({}, {})",
        plan.cluster_name, plan.network, plan.kubernetes
    );
    println!("Mode: {}", mode_name(mode));
    println!("VMs to create:");
    let mut any = false;
    for node in plan.bootstrap_nodes(include_control_plane) {
        if !current.contains(&node.name) {
            any = true;
            println!(
                "  - {} [{}] image={}",
                node.name,
                role_name(node.role),
                node.image
            );
        }
    }
    if !any {
        println!("  none");
    }

    println!("Bootstrap nodes:");
    for node in plan.bootstrap_nodes(include_control_plane) {
        println!(
            "  - {} [{}] pool={}",
            node.name,
            role_name(node.role),
            node.pool
        );
    }

    println!("Kubernetes metadata:");
    for node in plan.bootstrap_nodes(include_control_plane) {
        let labels = node
            .labels
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        let taint = node.taint.as_deref().unwrap_or("none");
        println!("  - {} labels=[{}] taint={}", node.name, labels, taint);
    }

    if let Some(path) = manifests {
        println!("Deploy manifests: {}", path.display());
    }
}

fn print_vm_status(plan: &FleetPlan, current: &BTreeSet<String>) {
    println!("exe.dev VMs:");
    for node in &plan.nodes {
        let state = if current.contains(&node.name) {
            "present"
        } else {
            "missing"
        };
        println!("  - {} [{}] {}", node.name, role_name(node.role), state);
    }
}

async fn print_kubernetes_status(plan: &FleetPlan, kubeconfig: Option<&Path>) -> Result<()> {
    let output = kubectl_capture(kubeconfig, &["get", "nodes", "-o", "json"]).await?;
    let nodes = parse_kubernetes_nodes(&output)?;
    println!("Kubernetes nodes:");
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
                expected.name,
                actual.ready,
                ok_text(labels_ok),
                ok_text(taint_ok)
            );
        } else {
            println!("  - {} missing", expected.name);
        }
    }
    Ok(())
}

async fn create_missing_vms(
    client: &ExeDevClient,
    plan: &FleetPlan,
    include_control_plane: bool,
    current: &BTreeSet<String>,
) -> Result<()> {
    for node in plan.bootstrap_nodes(include_control_plane) {
        if current.contains(&node.name) {
            continue;
        }
        let command = exe_new_command(node);
        println!("exe.dev: {command}");
        client.exec(&command).await?;
    }
    Ok(())
}

async fn bootstrap_k3s(
    plan: &FleetPlan,
    mode: ClusterMode,
    ts_authkey: &str,
    kubeconfig_arg: Option<&Path>,
) -> Result<()> {
    match mode {
        ClusterMode::New => {
            let control = plan
                .control_plane()
                .context("fleet has no control-plane node")?;
            let token = read_or_create_k3s_token(&plan.cluster_name)?;
            install_tailscale(&control.name, ts_authkey).await?;
            install_k3s_server(&control.name, &token).await?;
            let control_ip = remote_capture(&control.name, "tailscale ip -4 | head -n1").await?;
            let control_ip = control_ip.trim();
            if control_ip.is_empty() {
                bail!("failed to detect Tailscale IPv4 for {}", control.name);
            }
            let k3s_url = format!("https://{control_ip}:6443");
            let kubeconfig = fetch_kubeconfig(&control.name, control_ip).await?;
            let kubeconfig_path = kubeconfig_arg
                .map(Path::to_path_buf)
                .unwrap_or_else(|| generated_kubeconfig_path(&plan.cluster_name));
            write_secret_file(&kubeconfig_path, &kubeconfig)?;
            println!("Wrote kubeconfig: {}", kubeconfig_path.display());

            for node in plan
                .nodes
                .iter()
                .filter(|node| node.role != NodeRole::ControlPlane)
            {
                install_tailscale(&node.name, ts_authkey).await?;
                install_k3s_agent(&node.name, &k3s_url, &token).await?;
            }
        }
        ClusterMode::Existing => {
            let k3s_url = require_env(K3S_URL_ENV)?;
            let token = require_env(K3S_TOKEN_ENV)?;
            if kubeconfig_arg.is_none() && env::var_os("KUBECONFIG").is_none() {
                println!(
                    "warning: no --kubeconfig or KUBECONFIG set; kubectl will use its default config"
                );
            }
            for node in plan
                .nodes
                .iter()
                .filter(|node| node.role != NodeRole::ControlPlane)
            {
                install_tailscale(&node.name, ts_authkey).await?;
                install_k3s_agent(&node.name, &k3s_url, &token).await?;
            }
        }
    }
    Ok(())
}

async fn install_tailscale(vm: &str, authkey: &str) -> Result<()> {
    let script = format!(
        "if ! command -v tailscale >/dev/null 2>&1; then curl -fsSL https://tailscale.com/install.sh | sh; fi; sudo tailscale up --auth-key {} --ssh --accept-routes",
        shell_single_quote(authkey)
    );
    remote_run(vm, &script).await
}

async fn install_k3s_server(vm: &str, token: &str) -> Result<()> {
    let script = format!(
        "if ! command -v k3s >/dev/null 2>&1; then curl -sfL https://get.k3s.io | sudo env K3S_TOKEN={} sh -s - server --write-kubeconfig-mode 644; fi",
        shell_single_quote(token)
    );
    remote_run(vm, &script).await
}

async fn install_k3s_agent(vm: &str, k3s_url: &str, token: &str) -> Result<()> {
    let script = format!(
        "if ! command -v k3s >/dev/null 2>&1; then curl -sfL https://get.k3s.io | sudo env K3S_URL={} K3S_TOKEN={} sh -s - agent; fi",
        shell_single_quote(k3s_url),
        shell_single_quote(token)
    );
    remote_run(vm, &script).await
}

async fn fetch_kubeconfig(vm: &str, control_ip: &str) -> Result<String> {
    let kubeconfig = remote_capture(vm, "sudo cat /etc/rancher/k3s/k3s.yaml").await?;
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

async fn kubectl_apply(kubeconfig: Option<&Path>, manifests: &Path) -> Result<()> {
    kubectl_run_owned(
        kubeconfig,
        vec!["apply".into(), "-f".into(), manifests.display().to_string()],
    )
    .await
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

fn generated_kubeconfig_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR).join(cluster_name).join("kubeconfig")
}

fn generated_token_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR).join(cluster_name).join("k3s-token")
}

fn read_or_create_k3s_token(cluster_name: &str) -> Result<String> {
    if let Ok(token) = env::var(K3S_TOKEN_ENV) {
        return Ok(token);
    }
    let path = generated_token_path(cluster_name);
    if path.exists() {
        return fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(|text| text.trim().to_string());
    }
    let token = random_token();
    write_secret_file(&path, &token)?;
    Ok(token)
}

fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(())
}

fn random_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

async fn remote_run(vm: &str, script: &str) -> Result<()> {
    run_command(
        "ssh",
        &["exe.dev", "ssh", vm, "sh", "-lc", script],
        Stdio::inherit(),
    )
    .await
}

async fn remote_capture(vm: &str, script: &str) -> Result<String> {
    capture_command("ssh", &["exe.dev", "ssh", vm, "sh", "-lc", script]).await
}

async fn kubectl_run_owned(kubeconfig: Option<&Path>, args: Vec<String>) -> Result<()> {
    let mut words = kubeconfig_args(kubeconfig);
    words.extend(args);
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    run_command("kubectl", &refs, Stdio::inherit()).await
}

async fn kubectl_capture(kubeconfig: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut words = kubeconfig_args(kubeconfig);
    words.extend(args.iter().map(|arg| (*arg).to_string()));
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    capture_command("kubectl", &refs).await
}

fn kubeconfig_args(kubeconfig: Option<&Path>) -> Vec<String> {
    kubeconfig
        .map(|path| vec!["--kubeconfig".into(), path.display().to_string()])
        .unwrap_or_default()
}

async fn ensure_tool(tool: &str) -> Result<()> {
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

async fn run_command(program: &str, args: &[&str], stdout: Stdio) -> Result<()> {
    println!("$ {}", display_command(program, args));
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

async fn capture_command(program: &str, args: &[&str]) -> Result<String> {
    println!("$ {}", display_command(program, args));
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
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

fn display_command(program: &str, args: &[&str]) -> String {
    let words = std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();
    shell::shell_join(&words)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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

fn ok_text(ok: bool) -> &'static str {
    if ok { "ok" } else { "drift" }
}

fn parse_vm_names(response: &str) -> Result<BTreeSet<String>> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return Ok(BTreeSet::new());
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        let mut names = BTreeSet::new();
        collect_vm_names_from_json(&value, &mut names);
        if !names.is_empty() {
            return Ok(names);
        }
        if let Some(output) = value.get("output").and_then(Value::as_str) {
            return Ok(parse_vm_names_from_text(output));
        }
    }
    Ok(parse_vm_names_from_text(trimmed))
}

fn collect_vm_names_from_json(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    names.insert(name.to_string());
                } else {
                    collect_vm_names_from_json(item, names);
                }
            }
        }
        Value::Object(object) => {
            for key in ["name", "vm", "vmname", "vmName"] {
                if let Some(name) = object.get(key).and_then(Value::as_str) {
                    names.insert(name.to_string());
                    return;
                }
            }
            for key in ["vms", "items", "data"] {
                if let Some(child) = object.get(key) {
                    collect_vm_names_from_json(child, names);
                }
            }
        }
        _ => {}
    }
}

fn parse_vm_names_from_text(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.to_ascii_lowercase().starts_with("name"))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[derive(Debug)]
struct KubernetesNode {
    ready: bool,
    labels: BTreeMap<String, String>,
    taints: BTreeSet<String>,
}

fn parse_kubernetes_nodes(response: &str) -> Result<BTreeMap<String, KubernetesNode>> {
    let value = serde_json::from_str::<Value>(response).context("kubectl returned invalid JSON")?;
    let mut nodes = BTreeMap::new();
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .context("kubectl nodes JSON did not contain items")?;
    for item in items {
        let name = item
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .context("node missing metadata.name")?
            .to_string();
        let labels = item
            .pointer("/metadata/labels")
            .and_then(Value::as_object)
            .map(|object| {
                object
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_string()))
                    })
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let ready = item
            .pointer("/status/conditions")
            .and_then(Value::as_array)
            .map(|conditions| {
                conditions.iter().any(|condition| {
                    condition.get("type").and_then(Value::as_str) == Some("Ready")
                        && condition.get("status").and_then(Value::as_str) == Some("True")
                })
            })
            .unwrap_or(false);
        let taints = item
            .pointer("/spec/taints")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|taint| {
                        let key = taint.get("key").and_then(Value::as_str)?;
                        let effect = taint.get("effect").and_then(Value::as_str)?;
                        let value = taint.get("value").and_then(Value::as_str).unwrap_or("");
                        Some(format!("{key}={value}:{effect}"))
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        nodes.insert(
            name,
            KubernetesNode {
                ready,
                labels,
                taints,
            },
        );
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::super::fleet::NodeSpec;
    use super::*;

    #[test]
    fn parses_vm_names_from_json_array() {
        let names = parse_vm_names(r#"[{"name":"a"},{"vmName":"b"},"c"]"#).unwrap();
        assert!(names.contains("a"));
        assert!(names.contains("b"));
        assert!(names.contains("c"));
    }

    #[test]
    fn ignores_standalone_json_strings_in_objects() {
        let names =
            parse_vm_names(r#"{"status":"running","message":"ready","items":[{"name":"vm1"}]}"#)
                .unwrap();
        assert_eq!(names.len(), 1);
        assert!(names.contains("vm1"));
    }

    #[test]
    fn parses_vm_names_from_output_text() {
        let names =
            parse_vm_names(r#"{"output":"NAME STATUS\nvm1 running\nvm2 stopped\n"}"#).unwrap();
        assert_eq!(names.len(), 2);
        assert!(names.contains("vm1"));
        assert!(names.contains("vm2"));
    }

    #[test]
    fn builds_exedev_new_command() {
        let node = NodeSpec {
            name: "p1-a-1".into(),
            role: NodeRole::Worker,
            pool: "project1-a".into(),
            image: "ubuntu:22.04".into(),
            labels: BTreeMap::new(),
            taint: None,
        };
        assert_eq!(
            exe_new_command(&node),
            "new --name p1-a-1 --image ubuntu:22.04 --no-email"
        );
    }

    #[test]
    fn parses_kubernetes_node_metadata() {
        let nodes = parse_kubernetes_nodes(
            r#"
{
  "items": [
    {
      "metadata": {
        "name": "p1-a-1",
        "labels": { "exedev.dev/project": "project1" }
      },
      "spec": {
        "taints": [
          { "key": "exedev.dev/pool", "value": "project1-a", "effect": "NoSchedule" }
        ]
      },
      "status": {
        "conditions": [
          { "type": "Ready", "status": "True" }
        ]
      }
    }
  ]
}
"#,
        )
        .unwrap();
        let node = nodes.get("p1-a-1").unwrap();
        assert!(node.ready);
        assert_eq!(node.labels["exedev.dev/project"], "project1");
        assert!(
            node.taints
                .contains("exedev.dev/pool=project1-a:NoSchedule")
        );
    }
}
