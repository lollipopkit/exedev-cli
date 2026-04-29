#!/usr/bin/env bash

set -euo pipefail

exedev-cli-printf() {
  local level="${1:-info}"
  shift || true
  local message="$*"
  case "$level" in
    section) printf '\n==> %s\n' "$message" ;;
    success) printf '[ok] %s\n' "$message" ;;
    warn) printf '[warn] %s\n' "$message" >&2 ;;
    error) printf '[error] %s\n' "$message" >&2 ;;
    *) printf '[info] %s\n' "$message" ;;
  esac
}

exedev-cli-ask() {
  local question="$1"
  local default_yes="${2:-false}"
  local prompt answer

  if [ "${EXEDEV_CLI_ASSUME_YES:-0}" = "1" ]; then
    exedev-cli-printf info "$question: yes (--yes)"
    return 0
  fi

  if [ "$default_yes" = "true" ] || [ "$default_yes" = "1" ]; then
    prompt="$question [Y/n] "
  else
    prompt="$question [y/N] "
  fi

  while true; do
    printf '%s' "$prompt" > /dev/tty
    IFS= read -r answer < /dev/tty || answer=""
    case "$answer" in
      [Yy]|[Yy][Ee][Ss]) return 0 ;;
      [Nn]|[Nn][Oo]) return 1 ;;
      "")
        if [ "$default_yes" = "true" ] || [ "$default_yes" = "1" ]; then
          return 0
        fi
        return 1
        ;;
      *) exedev-cli-printf warn "please answer yes or no" ;;
    esac
  done
}

exedev-cli-run() {
  exedev-cli-printf info "$*"
  "$@"
}

exedev-cli-require-root() {
  if [ "$(id -u)" -eq 0 ]; then
    SUDO=""
  elif command -v sudo >/dev/null 2>&1; then
    SUDO="sudo"
  else
    exedev-cli-printf error "root or sudo is required"
    exit 127
  fi
  export SUDO
}

exedev-cli-require-command() {
  local command="$1"
  if ! command -v "$command" >/dev/null 2>&1; then
    exedev-cli-printf error "required command not found: $command"
    exit 127
  fi
}

_exedev_cli_json_escape() {
  local value="$1"
  value=${value//\\/\\\\}
  value=${value//\"/\\\"}
  value=${value//$'\n'/\\n}
  value=${value//$'\r'/\\r}
  value=${value//$'\t'/\\t}
  printf '%s' "$value"
}

exedev-cli-api() {
  if [ -z "${EXEDEV_CLI_API_URL:-}" ] || [ -z "${EXEDEV_CLI_API_TOKEN:-}" ]; then
    exedev-cli-printf error "exedev-cli-api is unavailable: missing EXEDEV_CLI_API_URL or EXEDEV_CLI_API_TOKEN"
    return 127
  fi
  if [ "$#" -lt 1 ]; then
    exedev-cli-printf error "usage: exedev-cli-api <fnName> [params...]"
    return 2
  fi

  local fn_name="$1"
  shift
  local json params sep arg
  params=""
  sep=""
  for arg in "$@"; do
    params="${params}${sep}\"$(_exedev_cli_json_escape "$arg")\""
    sep=","
  done
  json="{\"fnName\":\"$(_exedev_cli_json_escape "$fn_name")\",\"params\":[${params}]}"

  curl -fsS \
    -H "Authorization: Bearer ${EXEDEV_CLI_API_TOKEN}" \
    -H "Content-Type: application/json" \
    --data "$json" \
    "$EXEDEV_CLI_API_URL"
}
