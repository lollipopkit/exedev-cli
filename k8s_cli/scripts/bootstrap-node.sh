#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/exedev-cli.sh
. "${SCRIPT_DIR}/lib/exedev-cli.sh"

: "${EXEDEV_VM_NAME:?missing EXEDEV_VM_NAME}"
: "${EXEDEV_NODE_ROLE:?missing EXEDEV_NODE_ROLE}"
: "${TS_AUTHKEY:?missing TS_AUTHKEY}"
: "${K3S_BOOTSTRAP_TOKEN:?missing K3S_BOOTSTRAP_TOKEN}"

K3S_CLUSTER_CIDR="${K3S_CLUSTER_CIDR:-10.244.0.0/16}"
K3S_SERVICE_CIDR="${K3S_SERVICE_CIDR:-10.245.0.0/16}"

install_bootstrap_dependencies() {
  if ! command -v curl >/dev/null 2>&1; then
    if command -v apt-get >/dev/null 2>&1; then
      ${SUDO} apt-get update
      ${SUDO} env DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl
    else
      exedev-cli-printf error "curl is required and apt-get is not available; only Debian/Ubuntu apt-based images are supported"
      exit 127
    fi
  fi
}

start_tailscaled() {
  if command -v systemctl >/dev/null 2>&1 && ${SUDO} systemctl enable --now tailscaled; then
    return 0
  fi
  if command -v service >/dev/null 2>&1 && ${SUDO} service tailscaled start; then
    return 0
  fi
  if command -v tailscaled >/dev/null 2>&1; then
    ${SUDO} mkdir -p /var/lib/tailscale /run/tailscale
    ${SUDO} sh -c 'nohup tailscaled --state=/var/lib/tailscale/tailscaled.state --socket=/run/tailscale/tailscaled.sock >/var/log/tailscaled.log 2>&1 &'
    sleep 2
    return 0
  fi
  exedev-cli-printf error "tailscaled is not installed"
  exit 127
}

check_tailnet_lock() {
  local report="${1:-}"
  local lock_output
  lock_output="$(tailscale lock status 2>&1 || true)"
  report="${report}
${lock_output}"
  if printf '%s\n' "$report" | grep -Eqi 'LOCKED OUT by tailnet-lock|this node is locked out'; then
    exedev-cli-printf warn "Tailnet Lock is enabled and this VM is locked out."
    printf '%s\n' "$report" >&2
    exedev-cli-printf warn "Run the tailscale lock sign command above on a trusted signing node."
    if exedev-cli-ask "I have signed ${EXEDEV_VM_NAME}; retry tailscale up now?" false; then
      return 1
    fi
    exit 126
  fi
  if printf '%s\n' "$lock_output" | grep -qi 'Tailnet Lock is ENABLED'; then
    exedev-cli-printf info "Tailnet Lock is enabled; this VM is signed or otherwise accessible."
  fi
  return 0
}

install_tailscale() {
  exedev-cli-printf section "Install and start Tailscale"
  if ! command -v tailscale >/dev/null 2>&1; then
    curl -fsSL https://tailscale.com/install.sh | ${SUDO} sh
  fi
  start_tailscaled

  local output status
  while true; do
    set +e
    output="$(${SUDO} tailscale up --auth-key "$TS_AUTHKEY" --ssh --accept-routes 2>&1)"
    status=$?
    set -e
    if [ -n "$output" ]; then
      printf '%s\n' "$output" >&2
    fi
    if [ "$status" -eq 0 ]; then
      check_tailnet_lock "$output" || continue
      break
    fi
    exit "$status"
  done
}

tailscale_ipv4() {
  local ip
  ip="$(tailscale ip -4 | head -n1)"
  if [ -z "$ip" ]; then
    exedev-cli-printf error "failed to detect Tailscale IPv4"
    exit 1
  fi
  printf '%s' "$ip"
}

has_k3s_supervisor() {
  { command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; } || [ -x /sbin/openrc-run ]
}

start_k3s_service_no_block() {
  local service_name="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl enable "$service_name" >/dev/null 2>&1 || true
    ${SUDO} systemctl start --no-block "$service_name"
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} rc-update add "$service_name" default >/dev/null 2>&1 || true
    ${SUDO} service "$service_name" start
  else
    return 1
  fi
}

restart_k3s_service_no_block() {
  local service_name="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl enable "$service_name" >/dev/null 2>&1 || true
    ${SUDO} systemctl restart --no-block "$service_name"
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} rc-update add "$service_name" default >/dev/null 2>&1 || true
    ${SUDO} service "$service_name" restart
  else
    return 1
  fi
}

k3s_service_started() {
  local service_name="$1"
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    local state
    state="$(${SUDO} systemctl is-active "$service_name" 2>/dev/null || true)"
    [ "$state" = "active" ] || [ "$state" = "activating" ]
  elif [ -x /sbin/openrc-run ]; then
    ${SUDO} service "$service_name" status >/dev/null 2>&1
  else
    return 1
  fi
}

k3s_service_present_or_running() {
  local service_name="$1"
  if k3s_service_started "$service_name"; then
    return 0
  fi
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    ${SUDO} systemctl cat "$service_name" >/dev/null 2>&1
  elif [ -x /sbin/openrc-run ]; then
    [ -f "/etc/init.d/$service_name" ]
  else
    return 1
  fi
}

require_no_k3s_server_state_for_agent() {
  if k3s_service_present_or_running k3s || [ -d /var/lib/rancher/k3s/server ]; then
    exedev-cli-printf error "this VM already has k3s server state, but the fleet expects a worker node here"
    exit 1
  fi
}

require_no_k3s_agent_state_for_server() {
  if k3s_service_present_or_running k3s-agent && ! k3s_service_present_or_running k3s; then
    exedev-cli-printf error "this VM already has k3s agent state, but the fleet expects a control-plane node here"
    exit 1
  fi
}

print_k3s_service_logs() {
  local service_name="$1"
  if command -v journalctl >/dev/null 2>&1; then
    ${SUDO} journalctl -u "$service_name" -n 80 --no-pager >&2 || true
  fi
}

install_k3s_binary() {
  if command -v k3s >/dev/null 2>&1; then
    return 0
  fi

  local uname_arch k3s_arch k3s_suffix k3s_version tmp_bin tmp_hash base_url expected actual
  uname_arch="$(uname -m)"
  case "$uname_arch" in
    amd64|x86_64) k3s_arch="amd64"; k3s_suffix="" ;;
    arm64|aarch64) k3s_arch="arm64"; k3s_suffix="-arm64" ;;
    arm*) k3s_arch="arm"; k3s_suffix="-armhf" ;;
    *)
      exedev-cli-printf error "unsupported architecture for k3s: $uname_arch"
      exit 1
      ;;
  esac

  k3s_version="$(curl -w '%{url_effective}' -L -s -S https://update.k3s.io/v1-release/channels/stable -o /dev/null | sed -e 's|.*/||')"
  if [ -z "$k3s_version" ]; then
    exedev-cli-printf error "failed to resolve stable k3s version"
    exit 1
  fi

  tmp_bin="/tmp/exedev-k8s-k3s.$$"
  tmp_hash="/tmp/exedev-k8s-k3s.sha256.$$"
  base_url="https://github.com/k3s-io/k3s/releases/download/${k3s_version}"
  curl -sfL -o "$tmp_bin" "${base_url}/k3s${k3s_suffix}"
  curl -sfL -o "$tmp_hash" "${base_url}/sha256sum-${k3s_arch}.txt"

  expected="$(grep " k3s${k3s_suffix}$" "$tmp_hash" | awk '{print $1}')"
  actual="$(sha256sum "$tmp_bin" | awk '{print $1}')"
  if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    exedev-cli-printf error "k3s binary checksum verification failed"
    rm -f "$tmp_bin" "$tmp_hash"
    exit 1
  fi

  ${SUDO} mkdir -p /usr/local/bin
  chmod 755 "$tmp_bin"
  ${SUDO} chown root:root "$tmp_bin"
  ${SUDO} mv -f "$tmp_bin" /usr/local/bin/k3s
  rm -f "$tmp_hash"

  local tool
  for tool in kubectl crictl ctr; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      ${SUDO} ln -sf /usr/local/bin/k3s "/usr/local/bin/$tool"
    fi
  done
}

common_server_args() {
  printf '%s\n' \
    server \
    --write-kubeconfig-mode 644 \
    --node-name "$EXEDEV_VM_NAME" \
    --node-ip "$K3S_NODE_IP" \
    --advertise-address "$K3S_NODE_IP" \
    --tls-san "$K3S_TLS_SAN" \
    --cluster-cidr "$K3S_CLUSTER_CIDR" \
    --service-cidr "$K3S_SERVICE_CIDR"
}

install_k3s_server() {
  local server_mode="$1"
  local node_ip="$2"
  K3S_NODE_IP="$node_ip"
  K3S_TLS_SAN="${K3S_TLS_SAN:-$node_ip}"
  export K3S_NODE_IP K3S_TLS_SAN
  require_no_k3s_agent_state_for_server

  local args
  args="$(common_server_args)"
  if [ "$server_mode" = "primary" ]; then
    args="${args}
--cluster-init"
  else
    : "${K3S_SERVER_URL:?missing K3S_SERVER_URL for joining control-plane}"
    args="${args}
--server
${K3S_SERVER_URL}"
  fi

  exedev-cli-printf section "Install k3s server (${server_mode})"
  if has_k3s_supervisor; then
    if ! command -v k3s >/dev/null 2>&1; then
      # shellcheck disable=SC2086
      curl -sfL https://get.k3s.io | ${SUDO} env INSTALL_K3S_SKIP_START=true K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" sh -s - $args
    fi
    start_k3s_service_no_block k3s
  else
    install_k3s_binary
    if ! [ -f /var/run/exedev-k8s-k3s-server.pid ] || ! ${SUDO} kill -0 "$(${SUDO} cat /var/run/exedev-k8s-k3s-server.pid)" 2>/dev/null; then
      # shellcheck disable=SC2086
      ${SUDO} env K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" nohup k3s $args >/tmp/exedev-k8s-k3s-server.log 2>&1 &
      echo $! | ${SUDO} tee /var/run/exedev-k8s-k3s-server.pid >/dev/null
    fi
  fi

  local wait_count=0
  while [ "$wait_count" -lt 60 ]; do
    if [ -s /etc/rancher/k3s/k3s.yaml ]; then
      return 0
    fi
    wait_count=$((wait_count + 1))
    sleep 2
  done
  exedev-cli-printf error "k3s server did not write kubeconfig"
  if has_k3s_supervisor; then
    print_k3s_service_logs k3s
  fi
  if [ -f /tmp/exedev-k8s-k3s-server.log ]; then
    ${SUDO} tail -n 80 /tmp/exedev-k8s-k3s-server.log >&2 || true
  fi
  exit 1
}

install_k3s_agent() {
  local node_ip="$1"
  : "${K3S_SERVER_URL:?missing K3S_SERVER_URL for worker}"
  require_no_k3s_server_state_for_agent

  exedev-cli-printf section "Install k3s agent"
  if has_k3s_supervisor; then
    curl -sfL https://get.k3s.io | ${SUDO} env INSTALL_K3S_SKIP_START=true K3S_URL="$K3S_SERVER_URL" K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" sh -s - agent --node-name "$EXEDEV_VM_NAME" --node-ip "$node_ip"
    restart_k3s_service_no_block k3s-agent
  else
    install_k3s_binary
    if ! [ -f /var/run/exedev-k8s-k3s-agent.pid ] || ! ${SUDO} kill -0 "$(${SUDO} cat /var/run/exedev-k8s-k3s-agent.pid)" 2>/dev/null; then
      ${SUDO} env K3S_URL="$K3S_SERVER_URL" K3S_TOKEN="$K3S_BOOTSTRAP_TOKEN" nohup k3s agent --node-name "$EXEDEV_VM_NAME" --node-ip "$node_ip" >/tmp/exedev-k8s-k3s-agent.log 2>&1 &
      echo $! | ${SUDO} tee /var/run/exedev-k8s-k3s-agent.pid >/dev/null
    fi
  fi

  local wait_count=0
  while [ "$wait_count" -lt 30 ]; do
    if [ -f /var/run/exedev-k8s-k3s-agent.pid ] && ${SUDO} kill -0 "$(${SUDO} cat /var/run/exedev-k8s-k3s-agent.pid)" 2>/dev/null; then
      return 0
    fi
    if has_k3s_supervisor && k3s_service_started k3s-agent; then
      return 0
    fi
    wait_count=$((wait_count + 1))
    sleep 2
  done
  exedev-cli-printf error "k3s agent did not start"
  if has_k3s_supervisor; then
    print_k3s_service_logs k3s-agent
  elif [ -f /tmp/exedev-k8s-k3s-agent.log ]; then
    ${SUDO} tail -n 80 /tmp/exedev-k8s-k3s-agent.log >&2 || true
  fi
  exit 1
}

main() {
  exedev-cli-require-root
  install_bootstrap_dependencies
  install_tailscale
  local node_ip
  node_ip="$(tailscale_ipv4)"
  exedev-cli-printf info "Tailscale IPv4: ${node_ip}"

  case "$EXEDEV_NODE_ROLE" in
    control-plane-primary) install_k3s_server primary "$node_ip" ;;
    control-plane-join) install_k3s_server join "$node_ip" ;;
    worker) install_k3s_agent "$node_ip" ;;
    *)
      exedev-cli-printf error "unknown EXEDEV_NODE_ROLE: $EXEDEV_NODE_ROLE"
      exit 2
      ;;
  esac
  exedev-cli-printf success "node bootstrap complete: ${EXEDEV_VM_NAME}"
}

main "$@"
