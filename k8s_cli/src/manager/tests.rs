use super::super::fleet::NodeSpec;
use super::kubectl::kubeconfig_args;
use super::parsing::{
    parse_kubernetes_nodes, parse_ssh_destinations, parse_vm_names, parse_vm_names_from_text,
};
use super::process::{
    SshTargets, command_output_detail, display_command, parse_remote_stdout, remote_ssh_args,
    remote_status_script,
};
use super::scripts::{
    k3s_agent_install_command, k3s_server_install_command, tailscale_install_command,
};
use super::state::{
    generated_kubeconfig_path, generated_token_path, read_regular_file, write_secret_file,
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
fn parses_vm_names_from_output_wrapped_json() {
    let names = parse_vm_names(
        r#"{"output":"[{\"vm_name\":\"vm-1\",\"ssh_dest\":\"vm+vm-1@exe.dev\"},{\"vm_name\":\"vm-2\"}]"}"#,
    )
    .unwrap();
    assert_eq!(names.len(), 2);
    assert!(names.contains("vm-1"));
    assert!(names.contains("vm-2"));
}

#[test]
fn builds_exedev_new_command() {
    let node = NodeSpec {
        name: "p1-a-1".into(),
        role: NodeRole::Worker,
        pool: "project1-a".into(),
        image: "ubuntu:22.04".into(),
        cpu: None,
        memory: None,
        tags: Vec::new(),
        labels: BTreeMap::new(),
        taint: None,
    };
    assert_eq!(
        exe_new_command(&node),
        "new --name p1-a-1 --image ubuntu:22.04 --no-email"
    );
}

#[test]
fn builds_exedev_new_command_with_resources_and_tags() {
    let node = NodeSpec {
        name: "p1-a-1".into(),
        role: NodeRole::Worker,
        pool: "project1-a".into(),
        image: "exeuntu".into(),
        cpu: Some(4),
        memory: Some("16GB".into()),
        tags: vec!["k8s".into(), "prod".into()],
        labels: BTreeMap::new(),
        taint: None,
    };
    assert_eq!(
        exe_new_command(&node),
        "new --name p1-a-1 --image exeuntu --cpu 4 --memory 16GB --tag k8s --tag prod --no-email"
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
    // 600, not 644: k3s.yaml holds client credentials and fetch_kubeconfig reads
    // it through sudo, so nothing needs it world-readable on the VM.
    assert!(command.contains("--write-kubeconfig-mode 600 --node-name \"$K3S_NODE_NAME\""));
    assert!(!command.contains("--write-kubeconfig-mode 644"));
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
    let args = remote_ssh_args("vm-1.exe.xyz");
    assert_eq!(args.len(), 12);
    assert_eq!(args[0], "-o");
    assert_eq!(args[1], "ControlMaster=no");
    assert_eq!(args[2], "-o");
    assert_eq!(args[3], "ControlPath=none");
    assert_eq!(args[4], "-o");
    assert_eq!(args[5], "StrictHostKeyChecking=accept-new");
    assert_eq!(args[6], "-o");
    assert_eq!(args[7], "ConnectTimeout=15");
    assert_eq!(args[8], "--");
    assert_eq!(args[9], "vm-1.exe.xyz");
    assert_eq!(args[10], "sh");
    assert_eq!(args[11], "-s");
}

#[test]
fn option_shaped_destination_stays_a_destination() {
    let args = remote_ssh_args("-oProxyCommand=touch /tmp/pwned");
    let separator = args.iter().position(|arg| arg == "--").unwrap();
    assert_eq!(args[separator + 1], "-oProxyCommand=touch /tmp/pwned");
}

#[test]
fn parses_ssh_destinations_from_ls_json() {
    let destinations = parse_ssh_destinations(
        r#"{"vms":[
            {"vm_name":"routable","ssh_dest":"routable.exe.xyz","ssh_host":"routable.exe.xyz"},
            {"vm_name":"prefixed","ssh_dest":"vm+prefixed@exe.dev","ssh_host":"exe.dev","ssh_user":"vm+prefixed"},
            {"vm_name":"host-only","ssh_host":"shard3.exe.dev","ssh_user":"vm+host-only"},
            {"vm_name":"unknown"}
        ]}"#,
    );
    assert_eq!(destinations.get("routable").unwrap(), "routable.exe.xyz");
    assert_eq!(destinations.get("prefixed").unwrap(), "vm+prefixed@exe.dev");
    assert_eq!(
        destinations.get("host-only").unwrap(),
        "vm+host-only@shard3.exe.dev"
    );
    assert!(!destinations.contains_key("unknown"));
}

#[test]
fn parses_ssh_destinations_from_output_wrapped_json() {
    let destinations = parse_ssh_destinations(
        r#"{"output":"[{\"vm_name\":\"vm-1\",\"ssh_dest\":\"vm+vm-1@exe.dev\"}]"}"#,
    );
    assert_eq!(destinations.get("vm-1").unwrap(), "vm+vm-1@exe.dev");
}

#[test]
fn ignores_output_wrapped_table_text() {
    let destinations =
        parse_ssh_destinations(r#"{"output":"NAME STATUS\nvm1 running\nvm2 stopped\n"}"#);
    assert!(destinations.is_empty());
}

#[test]
fn ssh_targets_fall_back_to_exe_xyz_hostname() {
    let targets = SshTargets::new(parse_ssh_destinations(
        r#"[{"vm_name":"vm-1","ssh_dest":"vm+vm-1@exe.dev"}]"#,
    ));
    assert_eq!(targets.dest("vm-1"), "vm+vm-1@exe.dev");
    assert_eq!(targets.dest("vm-2"), "vm-2.exe.xyz");
    assert_eq!(SshTargets::default().dest("vm-3"), "vm-3.exe.xyz");
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

#[test]
fn secret_files_are_never_group_or_world_readable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("exedev-k8s-secret-{}", std::process::id()));
    let path = dir.join("k3s-token");
    let _ = std::fs::remove_dir_all(&dir);

    write_secret_file(&path, "first").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "fresh secret file mode");

    // Rewriting must not inherit the mode of whatever was there before.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    write_secret_file(&path, "second").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "rewritten secret file mode");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");

    // A symlink at the destination is replaced, not followed.
    let elsewhere = dir.join("elsewhere");
    std::fs::write(&elsewhere, "untouched").unwrap();
    let link = dir.join("linked-kubeconfig");
    std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
    write_secret_file(&link, "secret").unwrap();
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "untouched");
    assert_eq!(std::fs::read_to_string(&link).unwrap(), "secret");
    assert!(!std::fs::symlink_metadata(&link).unwrap().is_symlink());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn empty_json_listing_yields_no_vm_names() {
    assert!(parse_vm_names(r#"{"vms":[]}"#).unwrap().is_empty());
    assert!(parse_vm_names("[]").unwrap().is_empty());
    assert!(
        parse_vm_names(r#"{"error":"quota exceeded"}"#)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn text_fallback_keeps_vm_names_starting_with_name() {
    let names = parse_vm_names_from_text("NAME STATUS\nnameserver running\nvm2 stopped\n");
    assert!(names.contains("nameserver"));
    assert!(names.contains("vm2"));
    assert!(!names.contains("NAME"));
    assert_eq!(names.len(), 2);
}

#[test]
fn secret_write_failure_leaves_the_previous_secret_intact() {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join(format!("exedev-k8s-failpath-{}", std::process::id()));
    let path = dir.join("k3s-token");
    let _ = std::fs::remove_dir_all(&dir);
    write_secret_file(&path, "good-token").unwrap();

    // A read-only directory fails the staged create, standing in for any I/O
    // error partway through replacing the file.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let err = write_secret_file(&path, "replacement").unwrap_err();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();

    assert!(err.to_string().contains("failed to create"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "good-token");
    let staged = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(staged, 0, "staged file left behind");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn symlinked_token_is_rejected_rather_than_followed() {
    let dir = std::env::temp_dir().join(format!("exedev-k8s-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let secret_elsewhere = dir.join("other-secret");
    std::fs::write(&secret_elsewhere, "someone-elses-secret").unwrap();
    let token_path = dir.join("k3s-token");
    std::os::unix::fs::symlink(&secret_elsewhere, &token_path).unwrap();

    let err = read_regular_file(&token_path).unwrap_err();
    assert!(err.to_string().contains("not a regular file"));

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn wrapped_empty_listing_invents_no_vm_name() {
    assert!(parse_vm_names(r#"{"output":"[]"}"#).unwrap().is_empty());
    assert!(
        parse_vm_names(r#"{"output":"{\"error\":\"quota exceeded\"}"}"#)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cluster_name_cannot_escape_the_state_directory() {
    let path = generated_token_path("../../outside");
    assert!(path.starts_with(".exedev-k8s"));
    assert!(!path.to_string_lossy().contains(".."));
    assert!(generated_kubeconfig_path("../../outside").starts_with(".exedev-k8s"));
    assert_eq!(
        generated_token_path("prod-1"),
        Path::new(".exedev-k8s/prod-1/k3s-token")
    );
}

#[test]
fn staging_names_do_not_repeat() {
    let dir = std::env::temp_dir().join(format!("exedev-k8s-staging-{}", std::process::id()));
    let path = dir.join("k3s-token");
    let _ = std::fs::remove_dir_all(&dir);
    write_secret_file(&path, "one").unwrap();
    write_secret_file(&path, "two").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
    let leftovers = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
        .count();
    assert_eq!(leftovers, 0);
    std::fs::remove_dir_all(&dir).unwrap();
}
