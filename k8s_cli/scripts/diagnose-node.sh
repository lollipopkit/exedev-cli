#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/exedev-cli.sh
. "${SCRIPT_DIR}/lib/exedev-cli.sh"

exedev-cli-require-root

exedev-cli-printf section "k3s readyz"
${SUDO} k3s kubectl get --raw=/readyz 2>&1 || true

exedev-cli-printf section "k3s services"
if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
  ${SUDO} systemctl is-active k3s 2>&1 || true
  ${SUDO} systemctl status k3s --no-pager -l 2>&1 | tail -n 40 || true
  ${SUDO} systemctl is-active k3s-agent 2>&1 || true
  ${SUDO} systemctl status k3s-agent --no-pager -l 2>&1 | tail -n 40 || true
fi

exedev-cli-printf section "listeners on 6443"
if command -v ss >/dev/null 2>&1; then
  ${SUDO} ss -ltnp 2>&1 | grep ':6443' || true
elif command -v netstat >/dev/null 2>&1; then
  ${SUDO} netstat -ltnp 2>&1 | grep ':6443' || true
else
  exedev-cli-printf warn "ss/netstat not available"
fi

exedev-cli-printf section "tailscale"
tailscale ip -4 2>&1 || true
tailscale status --self 2>&1 || true
