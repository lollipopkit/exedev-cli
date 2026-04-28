pub(super) const REMOTE_PRIVILEGE_PRELUDE: &str = r#"
if [ "$(id -u)" -eq 0 ]; then
  SUDO=""
elif command -v sudo >/dev/null 2>&1; then
  SUDO="sudo"
else
  echo "root or sudo is required to bootstrap this VM" >&2
  exit 127
fi
"#;
pub(super) const REMOTE_BOOTSTRAP_PRELUDE: &str = r#"
if ! command -v curl >/dev/null 2>&1; then
  if command -v apt-get >/dev/null 2>&1; then
    ${SUDO} apt-get update
    ${SUDO} env DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl
  else
    echo "curl is required and apt-get is not available; only Debian/Ubuntu apt-based images are supported" >&2
    exit 127
  fi
fi
"#;
pub(super) const START_TAILSCALED_SCRIPT: &str = r#"
if command -v systemctl >/dev/null 2>&1 && ${SUDO} systemctl enable --now tailscaled; then
  :
elif command -v service >/dev/null 2>&1 && ${SUDO} service tailscaled start; then
  :
elif command -v tailscaled >/dev/null 2>&1; then
  ${SUDO} mkdir -p /var/lib/tailscale /run/tailscale
  ${SUDO} sh -c 'nohup tailscaled --state=/var/lib/tailscale/tailscaled.state --socket=/run/tailscale/tailscaled.sock >/var/log/tailscaled.log 2>&1 &'
  sleep 2
else
  echo "tailscaled is not installed" >&2
  exit 127
fi
"#;
pub(super) const CHECK_TAILNET_LOCK_SCRIPT: &str = r#"
tailnet_lock_report="${tailscale_up_output:-}
$(tailscale lock status 2>&1 || true)"
tailscale_lock_output="$(tailscale lock status 2>&1 || true)"
if printf '%s\n' "$tailnet_lock_report" | grep -Eqi 'LOCKED OUT by tailnet-lock|this node is locked out'; then
  echo "exedev-k8s bootstrap paused: Tailnet Lock is enabled and this VM is locked out." >&2
  echo "$tailnet_lock_report" >&2
  echo "Action required:" >&2
  echo "1. Run the tailscale lock sign command above on a trusted signing node." >&2
  echo "2. Rerun exedev-k8s bootstrap after signing is complete." >&2
  exit 126
fi
if printf '%s\n' "$tailscale_lock_output" | grep -qi 'Tailnet Lock is ENABLED'; then
  echo "tailscale: Tailnet Lock is enabled; this VM is signed or otherwise accessible." >&2
fi
"#;
pub(super) const K3S_INSTALL_HELPERS: &str = r#"
has_k3s_supervisor() {
  { command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; } || [ -x /sbin/openrc-run ]
}

start_k3s_service_no_block() {
  k3s_service="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl enable "$k3s_service" >/dev/null 2>&1 || true
    ${SUDO} systemctl start --no-block "$k3s_service"
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} rc-update add "$k3s_service" default >/dev/null 2>&1 || true
    ${SUDO} service "$k3s_service" start
  else
    return 1
  fi
}

restart_k3s_service_no_block() {
  k3s_service="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl enable "$k3s_service" >/dev/null 2>&1 || true
    ${SUDO} systemctl restart --no-block "$k3s_service"
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} rc-update add "$k3s_service" default >/dev/null 2>&1 || true
    ${SUDO} service "$k3s_service" restart
  else
    return 1
  fi
}

k3s_service_started() {
  k3s_service="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    k3s_state="$(${SUDO} systemctl is-active "$k3s_service" 2>/dev/null || true)"
    [ "$k3s_state" = "active" ] || [ "$k3s_state" = "activating" ]
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} service "$k3s_service" status >/dev/null 2>&1
  else
    return 1
  fi
}

k3s_service_present_or_running() {
  k3s_service="$1"
  if k3s_service_started "$k3s_service"; then
    return 0
  fi
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl cat "$k3s_service" >/dev/null 2>&1
  elif [ -x /sbin/openrc-run ]; then
    [ -f "/etc/init.d/$k3s_service" ]
  else
    return 1
  fi
}

require_no_k3s_server_state_for_agent() {
  if k3s_service_present_or_running k3s || [ -d /var/lib/rancher/k3s/server ]; then
    echo "this VM already has k3s server state, but the fleet expects a worker node here" >&2
    echo "delete/recreate the VM or run the k3s server uninstall script before bootstrapping it as a worker" >&2
    exit 1
  fi
}

require_no_k3s_agent_state_for_server() {
  if k3s_service_present_or_running k3s-agent && ! k3s_service_present_or_running k3s; then
    echo "this VM already has k3s agent state, but the fleet expects a control-plane node here" >&2
    echo "delete/recreate the VM or run the k3s agent uninstall script before bootstrapping it as a control-plane" >&2
    exit 1
  fi
}

print_k3s_service_logs() {
  k3s_service="$1"
  if command -v journalctl >/dev/null 2>&1; then
    ${SUDO} journalctl -u "$k3s_service" -n 80 --no-pager >&2 || true
  fi
}

install_k3s_binary() {
  if command -v k3s >/dev/null 2>&1; then
    return 0
  fi

  k3s_uname="$(uname -m)"
  case "$k3s_uname" in
    amd64|x86_64)
      k3s_arch="amd64"
      k3s_suffix=""
      ;;
    arm64|aarch64)
      k3s_arch="arm64"
      k3s_suffix="-arm64"
      ;;
    arm*)
      k3s_arch="arm"
      k3s_suffix="-armhf"
      ;;
    *)
      echo "unsupported architecture for k3s: $k3s_uname" >&2
      exit 1
      ;;
  esac

  k3s_version="$(curl -w '%{url_effective}' -L -s -S https://update.k3s.io/v1-release/channels/stable -o /dev/null | sed -e 's|.*/||')"
  if [ -z "$k3s_version" ]; then
    echo "failed to resolve stable k3s version" >&2
    exit 1
  fi

  k3s_tmp_bin="/tmp/exedev-k8s-k3s.$$"
  k3s_tmp_hash="/tmp/exedev-k8s-k3s.sha256.$$"
  k3s_base_url="https://github.com/k3s-io/k3s/releases/download/${k3s_version}"
  curl -sfL -o "$k3s_tmp_bin" "${k3s_base_url}/k3s${k3s_suffix}"
  curl -sfL -o "$k3s_tmp_hash" "${k3s_base_url}/sha256sum-${k3s_arch}.txt"

  k3s_expected="$(grep " k3s${k3s_suffix}$" "$k3s_tmp_hash" | awk '{print $1}')"
  k3s_actual="$(sha256sum "$k3s_tmp_bin" | awk '{print $1}')"
  if [ -z "$k3s_expected" ] || [ "$k3s_expected" != "$k3s_actual" ]; then
    echo "k3s binary checksum verification failed" >&2
    rm -f "$k3s_tmp_bin" "$k3s_tmp_hash"
    exit 1
  fi

  ${SUDO} mkdir -p /usr/local/bin
  chmod 755 "$k3s_tmp_bin"
  ${SUDO} chown root:root "$k3s_tmp_bin"
  ${SUDO} mv -f "$k3s_tmp_bin" /usr/local/bin/k3s
  rm -f "$k3s_tmp_hash"

  for tool in kubectl crictl ctr; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      ${SUDO} ln -sf /usr/local/bin/k3s "/usr/local/bin/$tool"
    fi
  done
}
"#;

pub(super) fn tailscale_install_command(authkey: &str) -> String {
    format!(
        "if ! command -v tailscale >/dev/null 2>&1; then curl -fsSL https://tailscale.com/install.sh | ${{SUDO}} sh; fi;\n{START_TAILSCALED_SCRIPT}\ntailscale_up_output=\"$(${{SUDO}} tailscale up --auth-key {} --ssh --accept-routes 2>&1)\"\ntailscale_up_status=$?\nif [ -n \"$tailscale_up_output\" ]; then\n  printf '%s\\n' \"$tailscale_up_output\" >&2\nfi\nif [ \"$tailscale_up_status\" -ne 0 ]; then\n  exit \"$tailscale_up_status\"\nfi\n{CHECK_TAILNET_LOCK_SCRIPT}",
        shell_single_quote(authkey)
    )
}

const K3S_CLUSTER_CIDR: &str = "10.244.0.0/16";
const K3S_SERVICE_CIDR: &str = "10.245.0.0/16";

pub(super) fn k3s_server_install_command(
    vm: &str,
    token: &str,
    tls_san: &str,
    node_ip: &str,
) -> String {
    format!(
        r#"K3S_BOOTSTRAP_TOKEN={}
K3S_NODE_NAME={}
K3S_TLS_SAN={}
K3S_NODE_IP={}
K3S_CLUSTER_CIDR={}
K3S_SERVICE_CIDR={}
{K3S_INSTALL_HELPERS}
require_no_k3s_agent_state_for_server
if has_k3s_supervisor; then
  if ! command -v k3s >/dev/null 2>&1; then
    curl -sfL https://get.k3s.io | ${{SUDO}} env INSTALL_K3S_SKIP_START=true K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" sh -s - server --write-kubeconfig-mode 644 --node-name "$K3S_NODE_NAME" --node-ip "$K3S_NODE_IP" --advertise-address "$K3S_NODE_IP" --tls-san "$K3S_TLS_SAN" --cluster-cidr "$K3S_CLUSTER_CIDR" --service-cidr "$K3S_SERVICE_CIDR"
  fi
  start_k3s_service_no_block k3s
else
  install_k3s_binary
  if ! [ -f /var/run/exedev-k8s-k3s-server.pid ] || ! ${{SUDO}} kill -0 "$(cat /var/run/exedev-k8s-k3s-server.pid)" 2>/dev/null; then
    ${{SUDO}} env K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" nohup k3s server --write-kubeconfig-mode 644 --node-name "$K3S_NODE_NAME" --node-ip "$K3S_NODE_IP" --advertise-address "$K3S_NODE_IP" --tls-san "$K3S_TLS_SAN" --cluster-cidr "$K3S_CLUSTER_CIDR" --service-cidr "$K3S_SERVICE_CIDR" >/tmp/exedev-k8s-k3s-server.log 2>&1 &
    echo $! | ${{SUDO}} tee /var/run/exedev-k8s-k3s-server.pid >/dev/null
  fi
fi

k3s_wait=0
while [ "$k3s_wait" -lt 60 ]; do
  if [ -s /etc/rancher/k3s/k3s.yaml ]; then
    break
  fi
  k3s_wait=$((k3s_wait + 1))
  sleep 2
done
if ! [ -s /etc/rancher/k3s/k3s.yaml ]; then
  echo "k3s server did not write kubeconfig" >&2
  if has_k3s_supervisor; then
    print_k3s_service_logs k3s
  fi
  if [ -f /tmp/exedev-k8s-k3s-server.log ]; then
    ${{SUDO}} tail -n 80 /tmp/exedev-k8s-k3s-server.log >&2 || true
  fi
  exit 1
fi"#,
        shell_single_quote(token),
        shell_single_quote(vm),
        shell_single_quote(tls_san),
        shell_single_quote(node_ip),
        shell_single_quote(K3S_CLUSTER_CIDR),
        shell_single_quote(K3S_SERVICE_CIDR)
    )
}

pub(super) fn k3s_agent_install_command(
    vm: &str,
    k3s_url: &str,
    token: &str,
    node_ip: &str,
) -> String {
    format!(
        r#"K3S_SERVER_URL={}
K3S_BOOTSTRAP_TOKEN={}
K3S_NODE_NAME={}
K3S_NODE_IP={}
{K3S_INSTALL_HELPERS}
require_no_k3s_server_state_for_agent
if has_k3s_supervisor; then
  curl -sfL https://get.k3s.io | ${{SUDO}} env INSTALL_K3S_SKIP_START=true K3S_URL="$K3S_SERVER_URL" K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" sh -s - agent --node-name "$K3S_NODE_NAME" --node-ip "$K3S_NODE_IP"
  restart_k3s_service_no_block k3s-agent
else
  install_k3s_binary
  if ! [ -f /var/run/exedev-k8s-k3s-agent.pid ] || ! ${{SUDO}} kill -0 "$(cat /var/run/exedev-k8s-k3s-agent.pid)" 2>/dev/null; then
    ${{SUDO}} env K3S_URL="$K3S_SERVER_URL" K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" nohup k3s agent --node-name "$K3S_NODE_NAME" --node-ip "$K3S_NODE_IP" >/tmp/exedev-k8s-k3s-agent.log 2>&1 &
    echo $! | ${{SUDO}} tee /var/run/exedev-k8s-k3s-agent.pid >/dev/null
  fi
fi

k3s_wait=0
while [ "$k3s_wait" -lt 30 ]; do
  if [ -f /var/run/exedev-k8s-k3s-agent.pid ] && ${{SUDO}} kill -0 "$(cat /var/run/exedev-k8s-k3s-agent.pid)" 2>/dev/null; then
    break
  fi
  if has_k3s_supervisor && k3s_service_started k3s-agent; then
    break
  fi
  k3s_wait=$((k3s_wait + 1))
  sleep 2
done
if ! has_k3s_supervisor; then
  if ! [ -f /var/run/exedev-k8s-k3s-agent.pid ] || ! ${{SUDO}} kill -0 "$(cat /var/run/exedev-k8s-k3s-agent.pid)" 2>/dev/null; then
    echo "k3s agent did not stay running" >&2
    if [ -f /tmp/exedev-k8s-k3s-agent.log ]; then
      ${{SUDO}} tail -n 80 /tmp/exedev-k8s-k3s-agent.log >&2 || true
    fi
    exit 1
  fi
elif ! k3s_service_started k3s-agent; then
  echo "k3s agent service did not start" >&2
  print_k3s_service_logs k3s-agent
  exit 1
fi"#,
        shell_single_quote(k3s_url),
        shell_single_quote(token),
        shell_single_quote(vm),
        shell_single_quote(node_ip)
    )
}

pub(super) fn remote_privileged_script(command: &str) -> String {
    format!("{REMOTE_PRIVILEGE_PRELUDE}\n{command}")
}

pub(super) fn remote_bootstrap_script(command: &str) -> String {
    remote_privileged_script(&format!("{REMOTE_BOOTSTRAP_PRELUDE}\n{command}"))
}

pub(super) fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
