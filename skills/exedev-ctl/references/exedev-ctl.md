# exedev-ctl Reference

## Install from GitHub Releases

Install `exedev-ctl` from the latest GitHub Release:

```sh
https://github.com/lollipopkit/exedev-cli/releases/latest
```

Release archives are named:

```text
exedev-clis-<tag>-linux-amd64.tar.gz
exedev-clis-<tag>-linux-arm64.tar.gz
exedev-clis-<tag>-macos-amd64.tar.gz
exedev-clis-<tag>-macos-arm64.tar.gz
```

Each archive contains both the `exedev-ctl` and `exedev-k8s` binaries, plus
`README.md`, `LICENSE`, `.env.example`, and `fleet.example.yaml`. Archive
member names carry a `./` prefix, so extract with `./exedev-ctl`, not
`exedev-ctl`.

Manual install pattern:

```sh
mkdir -p ~/.local/bin
tar -xzf exedev-clis-<tag>-<platform>.tar.gz ./exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

Make sure `~/.local/bin` is on `PATH`, or move the binary to another directory already on `PATH`, such as `/usr/local/bin`.

macOS Apple Silicon example:

```sh
tag="<release-tag>"   # for example v0.1.16
curl -L -o exedev-clis.tar.gz \
  "https://github.com/lollipopkit/exedev-cli/releases/download/${tag}/exedev-clis-${tag}-macos-arm64.tar.gz"
mkdir -p ~/.local/bin
tar -xzf exedev-clis.tar.gz ./exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

Linux amd64 example:

```sh
tag="<release-tag>"   # for example v0.1.16
curl -L -o exedev-clis.tar.gz \
  "https://github.com/lollipopkit/exedev-cli/releases/download/${tag}/exedev-clis-${tag}-linux-amd64.tar.gz"
mkdir -p ~/.local/bin
tar -xzf exedev-clis.tar.gz ./exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

Users who also want the k3s fleet CLI can extract `./exedev-k8s` from the same
archive; see `k8s_cli/README.md` in the source repository.

Verify installation:

```sh
exedev-ctl --help
exedev-ctl --json ls
```

Do not recommend `cargo run` or source-checkout workflows to third-party users unless they explicitly ask to build from source or maintain this repository.

## Installed Commands

After installation:

```sh
exedev-ctl --help
exedev-ctl --json ls
```

The CLI loads `.env` automatically, while shell environment values take precedence.

## Authentication

The CLI uses local SSH by default:

```sh
ssh exe.dev <command>
```

This mode does not need an API token.

HTTPS mode is explicit:

```sh
exedev-ctl --transport http ls
```

In HTTPS mode, the CLI reads:

```sh
EXE_DEV_API_KEY=exe0....
```

The token is used as Bearer auth against:

```text
POST https://exe.dev/exec
```

The request body is the native exe.dev command, equivalent to:

```sh
ssh exe.dev <command>
```

Use a dedicated SSH key for automation tokens where possible. Permission failures are commonly `403 command not allowed by token permissions`; the fix is usually to generate a token whose `cmds` include the needed command.

## Common VM Tasks

List VMs:

```sh
exedev-ctl --json ls
exedev-ctl --json ls --group tag
exedev-ctl --transport http --json ls
```

Create a VM:

```sh
exedev-ctl new --name p1-a-1 --image exeuntu --no-email
exedev-ctl new --name p1-a-1 --image exeuntu --cpu 4 --memory 16GB --tag prod --no-email
```

Prefer the `exeuntu` image for nodes that need `sudo`, `curl`, or systemd;
generic minimal images such as `ubuntu:22.04` may lack them.

Delete a VM:

```sh
exedev-ctl --yes rm p1-a-1
```

Restart a VM:

```sh
exedev-ctl restart p1-a-1
```

Rename a VM:

```sh
exedev-ctl rename old-name new-name
```

Tag or untag a VM (multiple tags per call are supported):

```sh
exedev-ctl tag p1-a-1 prod web
exedev-ctl tag -d p1-a-1 prod web
```

Set or clear a short comment on a VM:

```sh
exedev-ctl comment p1-a-1 "staging copy"
exedev-ctl comment p1-a-1 ""
```

Show VM metrics:

```sh
exedev-ctl stat p1-a-1
```

Resize resources (at least one of `--memory`, `--cpu`, `--disk`):

```sh
exedev-ctl resize p1-a-1 --disk 80G
exedev-ctl resize p1-a-1 --cpu 4 --memory 16GB
```

Share HTTP access:

```sh
exedev-ctl share show p1-a-1
exedev-ctl share port p1-a-1 8080
exedev-ctl share set-public p1-a-1
exedev-ctl share set-private p1-a-1
```

Share with specific users or via link:

```sh
exedev-ctl share add p1-a-1 user@example.com --message "check this"
exedev-ctl share remove p1-a-1 user@example.com
exedev-ctl share add-link p1-a-1
exedev-ctl share remove-link p1-a-1 <token>
```

Manage custom domains after DNS points at the VM:

```sh
exedev-ctl domain add p1-a-1 app.example.com
exedev-ctl domain ls p1-a-1
exedev-ctl domain ls -a
exedev-ctl domain rm p1-a-1 app.example.com
```

SSH into a VM:

```sh
exedev-ctl ssh p1-a-1
ssh p1-a-1.exe.xyz
```

Run raw exe.dev command:

```sh
exedev-ctl exec -- whoami
```

## Token Generation Helper

The `exedev-ctl` wrapper supports exe.dev token generation with `--label`, `--vm`, `--cmds`, and `--exp`:

```sh
exedev-ctl ssh-key generate-api-key --label automation --cmds "ls,new,whoami,share show,share port,domain add,domain ls,domain rm" --exp 30d
```

For a VM-scoped token accepted by the VM HTTPS proxy (not `/exec`):

```sh
exedev-ctl ssh-key generate-api-key --vm p1-a-1 --label deploy
```

When `--cmds` is omitted, exe.dev grants the defaults: `help`, `ls`, `new`, `whoami`, `ssh-key list`, `share show`, `exe0-to-exe1`, `team`, and `team members`. Only command names are checked; flags like `--json` are always allowed. For destructive operations, include commands intentionally and narrowly, for example `rm`, `restart`, or `rename` only when needed.

## Triage Checklist

When a VM task fails:

1. Run `exedev-ctl whoami` or `exedev-ctl --json ls` to verify default SSH auth.
2. For HTTPS-specific failures, retry with `exedev-ctl --transport http whoami` or `exedev-ctl --transport http --json ls`.
3. If HTTP status is `403`, check token `cmds` permissions.
4. If HTTP status is `422`, read the exe.dev command failure body.
5. If interactive SSH or stdin is involved, use the SSH path.
6. If SSH or script transport fails, prefer direct `ssh <vm>.exe.xyz ...` checks to separate VM reachability from exe.dev API permissions.
