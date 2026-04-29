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

Each archive contains the `exedev-ctl` binary.

Manual install pattern:

```sh
mkdir -p ~/.local/bin
tar -xzf exedev-clis-<tag>-<platform>.tar.gz exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

Make sure `~/.local/bin` is on `PATH`, or move the binary to another directory already on `PATH`, such as `/usr/local/bin`.

macOS Apple Silicon example:

```sh
tag="<release-tag>"
curl -L -o exedev-clis.tar.gz \
  "https://github.com/lollipopkit/exedev-cli/releases/download/${tag}/exedev-clis-${tag}-macos-arm64.tar.gz"
mkdir -p ~/.local/bin
tar -xzf exedev-clis.tar.gz exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

Linux amd64 example:

```sh
tag="<release-tag>"
curl -L -o exedev-clis.tar.gz \
  "https://github.com/lollipopkit/exedev-cli/releases/download/${tag}/exedev-clis-${tag}-linux-amd64.tar.gz"
mkdir -p ~/.local/bin
tar -xzf exedev-clis.tar.gz exedev-ctl
mv exedev-ctl ~/.local/bin/
chmod +x ~/.local/bin/exedev-ctl
```

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

The CLI reads:

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
```

Create a VM:

```sh
exedev-ctl new --name p1-a-1 --image ubuntu:22.04 --no-email
```

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

Tag or untag a VM:

```sh
exedev-ctl tag p1-a-1 role=worker
exedev-ctl tag -d p1-a-1 role=worker
```

Show VM metrics:

```sh
exedev-ctl stat p1-a-1
```

Resize disk:

```sh
exedev-ctl resize p1-a-1 --disk 80G
```

Share HTTP access:

```sh
exedev-ctl share show p1-a-1
exedev-ctl share port p1-a-1 8080
exedev-ctl share set-public p1-a-1
exedev-ctl share set-private p1-a-1
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

The `exedev-ctl` wrapper supports exe.dev token generation:

```sh
exedev-ctl ssh-key generate-api-key --label automation --cmds "ls,new,whoami,share show,share port" --exp 1798761600
```

For destructive operations, include commands intentionally and narrowly, for example `rm`, `restart`, or `rename` only when needed.

## Triage Checklist

When a VM task fails:

1. Run `exedev-ctl whoami` or `exedev-ctl --json ls` to verify token/auth.
2. If HTTP status is `403`, check token `cmds` permissions.
3. If HTTP status is `422`, read the exe.dev command failure body.
4. If interactive SSH or stdin is involved, switch to the SSH fallback path.
5. If SSH or script transport fails, prefer direct `ssh <vm>.exe.xyz ...` checks to separate VM reachability from exe.dev API permissions.
