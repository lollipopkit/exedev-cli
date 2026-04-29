---
name: exedev-ctl
description: Manage exe.dev virtual machines with the exedev-ctl CLI installed from GitHub Releases. Use when Codex needs to help third-party users install exedev-ctl, inspect, create, delete, restart, rename, tag, resize, SSH into, share, or troubleshoot VMs on exe.dev; generate or check EXE_DEV_API_KEY permissions; or run raw exe.dev commands.
---

# exedev-ctl

## Core Workflow

Use the installed `exedev-ctl` binary for exe.dev VM lifecycle and account-level operations:

```sh
exedev-ctl --help
exedev-ctl ls
```

For installation or platform-specific archive names, read `references/exedev-ctl.md`. Do not recommend `cargo run` to third-party users unless the user is explicitly maintaining the source repository.

## Install First

If `exedev-ctl` is not installed, direct users to the latest GitHub Release:

```sh
https://github.com/lollipopkit/exedev-cli/releases/latest
```

Release archives are named `exedev-clis-<tag>-<platform>.tar.gz` and include the `exedev-ctl` binary.

## Before Acting

Check the current environment and scope before proposing changes:

- Verify `EXE_DEV_API_KEY` is present for HTTPS `/exec` operations.
- Use `exedev-ctl --json ls` to inspect current VMs.
- Treat destructive VM actions as high risk. Require explicit confirmation before `rm`, bulk deletion, or operations that could lose disk state unless the user already asked for that exact action.
- When a token returns `403`, inspect token permissions before assuming a VM or CLI bug.
- When `/exec` returns `422`, surface the exe.dev command failure body.

## Command Selection

Use typed wrappers for supported commands: `ls`, `new`, `rm`, `restart`, `rename`, `tag`, `stat`, `cp`, `resize`, `share`, `team`, `whoami`, `ssh-key`, `set-region`, `integrations`, `billing`, `shelley`, `browser`, `ssh`, and `grant-support-root`.

Use `exec -- <command>` only for raw exe.dev commands that do not yet have a typed wrapper.

Use `--json` when output must be parsed, compared, or included in automation.

## SSH and API Boundary

`exedev-ctl` uses `POST https://exe.dev/exec` by default. The endpoint has no stdin or pty, so commands that need interactive behavior fall back to local SSH:

- `exedev-ctl ssh ...`
- `exedev-ctl new --prompt /dev/stdin`
- `exedev-ctl new --setup-script /dev/stdin`

For streamed scripts or interactive VM work, prefer direct VM SSH such as `ssh <vm>.exe.xyz ...` when the repo evidence shows it is the reliable path.

## More Detail

Read `references/exedev-ctl.md` when you need concrete command examples, token-generation guidance, error triage, or repo-specific checks.
