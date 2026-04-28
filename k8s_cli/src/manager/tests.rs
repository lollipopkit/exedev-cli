use super::super::fleet::NodeSpec;
use super::kubectl::kubeconfig_args;
use super::parsing::{parse_kubernetes_nodes, parse_vm_names};
use super::process::{
    command_output_detail, display_command, parse_remote_stdout, remote_ssh_args,
    remote_status_script,
};
use super::scripts::{
    k3s_agent_install_command, k3s_server_install_command, tailscale_install_command,
};
use super::*;
use std::{collections::BTreeMap, path::Path};

#[test]
fn parses_vm_names_from_json_array() {
    let names = parse_vm_names(r#"[{"name":"a"},{"vmName":"b"},{"vm_name":"d"},"c"]"#).unwrap();
    assert!(names.contains("a"));
    assert!(names.contains("b"));
    assert!(names.contains("c"));
    assert!(names.contains("d"));
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
    let names = parse_vm_names(r#"{"output":"NAME STATUS\nvm1 running\nvm2 stopped\n"}"#).unwrap();
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
fn tailscale_install_command_starts_daemon_before_up() {
    let command = tailscale_install_command("tskey-auth-test");
    let start_index = command.find("systemctl enable --now tailscaled").unwrap();
    let up_index = command.find("tailscale up --auth-key").unwrap();
    let lock_index = command.find("tailscale lock status").unwrap();
    assert!(start_index < up_index);
    assert!(up_index < lock_index);
    assert!(command.contains("service tailscaled start"));
    assert!(command.contains("nohup tailscaled"));
    assert!(command.contains("--auth-key 'tskey-auth-test'"));
    assert!(command.contains("tailscale_up_output=\"$(${SUDO} tailscale up"));
    assert!(command.contains("this node is locked out"));
    assert!(command.contains("LOCKED OUT by tailnet-lock"));
    assert!(command.contains("bootstrap paused"));
    assert!(command.contains("Action required:"));
    assert!(command.contains("trusted signing node"));
}

#[test]
fn k3s_server_install_command_supports_no_supervisor_fallback() {
    let command =
        k3s_server_install_command("vm-1", "token'with-quote", "100.64.0.10", "100.64.0.10");
    assert!(command.contains("[ -d /run/systemd/system ]"));
    assert!(command.contains("install_k3s_binary"));
    assert!(command.contains("nohup k3s server"));
    assert!(command.contains("INSTALL_K3S_SKIP_START=true"));
    assert!(command.contains("start_k3s_service_no_block k3s"));
    assert!(command.contains("systemctl start --no-block \"$k3s_service\""));
    assert!(command.contains("--write-kubeconfig-mode 644 --node-name \"$K3S_NODE_NAME\""));
    assert!(command.contains("require_no_k3s_agent_state_for_server"));
    assert!(command.contains("--cluster-cidr \"$K3S_CLUSTER_CIDR\""));
    assert!(command.contains("--service-cidr \"$K3S_SERVICE_CIDR\""));
    assert!(command.contains("--node-ip \"$K3S_NODE_IP\""));
    assert!(command.contains("--advertise-address \"$K3S_NODE_IP\""));
    assert!(command.contains("--tls-san \"$K3S_TLS_SAN\""));
    assert!(command.contains("K3S_BOOTSTRAP_TOKEN='token'\\''with-quote'"));
    assert!(command.contains("K3S_NODE_NAME='vm-1'"));
    assert!(command.contains("K3S_TLS_SAN='100.64.0.10'"));
    assert!(command.contains("K3S_NODE_IP='100.64.0.10'"));
    assert!(command.contains("K3S_CLUSTER_CIDR='10.244.0.0/16'"));
    assert!(command.contains("K3S_SERVICE_CIDR='10.245.0.0/16'"));
}

#[test]
fn k3s_agent_install_command_supports_no_supervisor_fallback() {
    let command =
        k3s_agent_install_command("vm-2", "https://100.64.0.1:6443", "token", "100.64.0.2");
    assert!(command.contains("install_k3s_binary"));
    assert!(command.contains("nohup k3s agent --node-name \"$K3S_NODE_NAME\""));
    assert!(command.contains("--node-ip \"$K3S_NODE_IP\""));
    assert!(command.contains("INSTALL_K3S_SKIP_START=true"));
    assert!(command.contains("require_no_k3s_server_state_for_agent"));
    assert!(command.contains("restart_k3s_service_no_block k3s-agent"));
    assert!(command.contains("systemctl restart --no-block \"$k3s_service\""));
    assert!(command.contains("k3s_service_started k3s-agent"));
    assert!(command.contains("K3S_SERVER_URL='https://100.64.0.1:6443'"));
    assert!(command.contains("K3S_BOOTSTRAP_TOKEN='token'"));
    assert!(command.contains("K3S_NODE_NAME='vm-2'"));
    assert!(command.contains("K3S_NODE_IP='100.64.0.2'"));
}

#[test]
fn builds_remote_ssh_command_for_stdin_script() {
    let args = remote_ssh_args("vm-1");
    assert_eq!(args.len(), 11);
    assert_eq!(args[0], "-o");
    assert_eq!(args[1], "ControlMaster=no");
    assert_eq!(args[2], "-o");
    assert_eq!(args[3], "ControlPath=none");
    assert_eq!(args[4], "-o");
    assert_eq!(args[5], "StrictHostKeyChecking=accept-new");
    assert_eq!(args[6], "-o");
    assert_eq!(args[7], "ConnectTimeout=15");
    assert_eq!(args[8], "vm-1.exe.xyz");
    assert_eq!(args[9], "sh");
    assert_eq!(args[10], "-s");
}

#[test]
fn display_command_redacts_bootstrap_secrets() {
    let command = display_command(
        "ssh",
        &[
            "exe.dev",
            "ssh vm-1 'sh -lc '\\''K3S_BOOTSTRAP_TOKEN='\\''\\'\\'''\\''abc123'\\''\\'\\'''\\''\nsudo tailscale up --auth-key '\\''\\'\\'''\\''tskey-auth-secret'\\''\\'\\'''\\'''\\'''",
        ],
    );
    assert!(command.contains("K3S_BOOTSTRAP_TOKEN=<redacted>"));
    assert!(command.contains("tskey-auth-<redacted>"));
    assert!(!command.contains("abc123"));
    assert!(!command.contains("tskey-auth-secret"));
}

#[test]
fn detects_vm_name_unavailable_response_body() {
    assert!(is_vm_name_unavailable_body(
        r#"{"error":"VM name \"test-min-ctl-1\" is not available"}"#,
        "test-min-ctl-1"
    ));
    assert!(!is_vm_name_unavailable_body(
        r#"{"error":"quota exceeded"}"#,
        "test-min-ctl-1"
    ));
}

#[test]
fn parses_remote_command_stdout_status_marker() {
    let (stdout, status) = parse_remote_stdout("vm-1", "hello\n__EXEDEV_K8S_EXIT__:7\n").unwrap();
    assert_eq!(stdout, "hello");
    assert_eq!(status, 7);
}

#[test]
fn rejects_remote_command_output_without_status_marker() {
    let err = parse_remote_stdout("vm-1", "hello\n").unwrap_err();
    assert!(
        err.to_string()
            .contains("remote command on vm-1 did not report an exit status")
    );
}

#[test]
fn command_output_detail_prefers_stderr_and_keeps_stdout_context() {
    let detail = command_output_detail(b"stdout detail\n", b"stderr detail\n");
    assert_eq!(detail, "stderr detail\nstdout detail");
    assert_eq!(command_output_detail(b"\n", b""), "");
}

#[test]
fn kubeconfig_args_include_selected_request_timeout() {
    let args = kubeconfig_args(Some(Path::new("cluster.yaml")), "12s");
    assert_eq!(
        args,
        vec![
            "--kubeconfig".to_string(),
            "cluster.yaml".to_string(),
            "--request-timeout=12s".to_string()
        ]
    );
}

#[test]
fn tailscale_policy_hint_mentions_node_and_local_permissions() {
    let hint = tailscale_policy_hint();
    assert!(hint.contains("tag:server -> tag:server tcp:6443"));
    assert!(hint.contains("local kubectl client"));
}

#[test]
fn wraps_remote_status_script_in_subshell() {
    let script = remote_status_script("vm-1", "echo ok");
    assert!(script.contains("__exedev_k8s_expected_hostname=vm-1"));
    assert!(script.contains("exedev-k8s target mismatch"));
    assert!(script.contains("(\necho ok\n  )\n"));
    assert!(script.contains("__EXEDEV_K8S_EXIT__:%s"));
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
