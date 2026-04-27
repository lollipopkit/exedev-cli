# exedevctl

[中文](README.zh-CN.md)

`exedevctl` is the Rust exe.dev CLI included in this workspace.

It calls exe.dev through the official HTTPS API by default:

```text
POST https://exe.dev/exec
```

The request body is the native exe.dev command, equivalent to
`ssh exe.dev <command>`.

## Build

```sh
cargo build -p exedevctl
```

Run from Cargo:

```sh
cargo run -p exedevctl -- --help
```

Or use the binary directly:

```sh
./target/debug/exedevctl --help
```

## Authentication

`exedevctl` reads `EXE_DEV_API_KEY`:

```sh
export EXE_DEV_API_KEY="exe0...."
```

See [`../docs/exedev-automation.md`](../docs/exedev-automation.md) for token
generation.

## Common Commands

List VMs:

```sh
exedevctl ls
```

Create a VM:

```sh
exedevctl new --name p1-a-1 --image ubuntu:22.04 --no-email
```

Delete a VM:

```sh
exedevctl rm p1-a-1
```

Skip confirmation for dangerous operations:

```sh
exedevctl --yes rm p1-a-1
```

Set the HTTP proxy port:

```sh
exedevctl share port p1-a-1 8080
```

Make the HTTP proxy public:

```sh
exedevctl share set-public p1-a-1
```

Run a raw exe.dev command:

```sh
exedevctl exec -- 'whoami'
```

## Output

The default output is optimized for humans.

Use `--json` to print raw JSON:

```sh
exedevctl --json ls
```

## SSH Fallback

The official HTTPS `/exec` endpoint has no pty and no stdin. These cases fall
back to the local command:

```sh
ssh exe.dev ...
```

Current fallback cases:

- `exedevctl ssh ...`
- `exedevctl new --prompt /dev/stdin`
- `exedevctl new --setup-script /dev/stdin`

These commands require local SSH access to exe.dev.

## Coverage

The CLI covers the top-level commands from the exe.dev CLI Reference:

```text
help doc ls new rm restart rename tag stat cp resize share team whoami
ssh-key set-region integrations billing shelley browser ssh grant-support-root
exit exec
```

`exec` is the fallback command for future exe.dev commands that do not yet have
a typed wrapper.
