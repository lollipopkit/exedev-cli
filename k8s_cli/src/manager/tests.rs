use super::super::fleet::NodeSpec;
use super::kubectl::kubeconfig_args;
use super::parsing::{parse_kubernetes_nodes, parse_vm_names};
use super::process::{
    command_output_detail, display_command, parse_remote_stdout, remote_interactive_command,
    remote_ssh_args, remote_status_script, scp_args,
};
use super::scripts::{BootstrapNodeRequest, BootstrapNodeRole};
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
fn builds_scp_command_for_script_upload() {
    let args = scp_args("k8s_cli/scripts/.", "vm-1.exe.xyz:/tmp/scripts");
    assert_eq!(args[0], "-r");
    assert!(args.contains(&"StrictHostKeyChecking=accept-new".to_string()));
    assert_eq!(args[args.len() - 2], "k8s_cli/scripts/.");
    assert_eq!(args[args.len() - 1], "vm-1.exe.xyz:/tmp/scripts");
}

#[test]
fn bootstrap_node_request_envs_include_role_and_assume_yes() {
    let request = BootstrapNodeRequest {
        vm_name: "vm-1".into(),
        role: BootstrapNodeRole::ControlPlaneJoin,
        cluster_name: "pg".into(),
        ts_authkey: "tskey-auth-test".into(),
        k3s_token: "token".into(),
        k3s_server_url: Some("https://100.64.0.1:6443".into()),
        assume_yes: true,
    };
    let envs = request.envs();
    assert!(envs.contains(&("EXEDEV_NODE_ROLE".into(), "control-plane-join".into())));
    assert!(envs.contains(&("EXEDEV_CLI_ASSUME_YES".into(), "1".into())));
    assert!(envs.contains(&("K3S_SERVER_URL".into(), "https://100.64.0.1:6443".into())));
}

#[test]
fn remote_interactive_command_exports_runtime_env() {
    let command = remote_interactive_command(
        "/tmp/exedev-k8s",
        "bootstrap-node.sh",
        &[
            ("EXEDEV_NODE_ROLE".into(), "worker".into()),
            ("K3S_BOOTSTRAP_TOKEN".into(), "token'quote".into()),
        ],
        None,
    );
    assert!(command.starts_with("cd /tmp/exedev-k8s && env "));
    assert!(command.contains("EXEDEV_NODE_ROLE=worker"));
    assert!(command.contains("'K3S_BOOTSTRAP_TOKEN=token'\\''quote'"));
    assert!(command.ends_with("bash ./bootstrap-node.sh"));
}

#[test]
fn display_command_redacts_bootstrap_secrets() {
    let command = display_command(
        "ssh",
        &[
            "exe.dev",
            "ssh vm-1 'sh -lc '\\''K3S_BOOTSTRAP_TOKEN='\\''\\'\\'''\\''abc123'\\''\\'\\'''\\''\nexample --auth-key '\\''\\'\\'''\\''tskey-auth-secret'\\''\\'\\'''\\'''\\'''",
        ],
    );
    assert!(command.contains("K3S_BOOTSTRAP_TOKEN=<redacted>"));
    assert!(
        display_command("ssh", &["EXEDEV_CLI_API_TOKEN=super-secret"])
            .contains("EXEDEV_CLI_API_TOKEN=<redacted>")
    );
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
