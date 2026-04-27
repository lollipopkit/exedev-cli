# exedevctl

[English](README.md)

`exedevctl` 是本 workspace 内置的 Rust exe.dev CLI。

它默认通过官方 HTTPS API 调用 exe.dev：

```text
POST https://exe.dev/exec
```

请求体是原生 exe.dev command，语义等同于：

```sh
ssh exe.dev <command>
```

## Build

```sh
cargo build -p exedevctl
```

通过 Cargo 运行：

```sh
cargo run -p exedevctl -- --help
```

或直接运行 binary：

```sh
./target/debug/exedevctl --help
```

## Authentication

`exedevctl` 读取 `EXE_DEV_API_KEY`：

```sh
export EXE_DEV_API_KEY="exe0...."
```

token generation 见
[`../docs/exedev-automation.md`](../docs/exedev-automation.md)。

## Common Commands

列出 VMs：

```sh
exedevctl ls
```

创建 VM：

```sh
exedevctl new --name p1-a-1 --image ubuntu:22.04 --no-email
```

删除 VM：

```sh
exedevctl rm p1-a-1
```

跳过危险操作确认：

```sh
exedevctl --yes rm p1-a-1
```

设置 HTTP proxy port：

```sh
exedevctl share port p1-a-1 8080
```

将 HTTP proxy 设为 public：

```sh
exedevctl share set-public p1-a-1
```

运行原始 exe.dev command：

```sh
exedevctl exec -- 'whoami'
```

## Output

默认 output 面向人类阅读优化。

使用 `--json` 打印 raw JSON：

```sh
exedevctl --json ls
```

## SSH Fallback

官方 HTTPS `/exec` endpoint 没有 pty，也没有 stdin。以下场景会 fallback 到本地命令：

```sh
ssh exe.dev ...
```

当前 fallback 场景：

- `exedevctl ssh ...`
- `exedevctl new --prompt /dev/stdin`
- `exedevctl new --setup-script /dev/stdin`

这些命令需要本机能够通过 SSH 访问 exe.dev。

## Coverage

CLI 覆盖 exe.dev CLI Reference 中的 top-level commands：

```text
help doc ls new rm restart rename tag stat cp resize share team whoami
ssh-key set-region integrations billing shelley browser ssh grant-support-root
exit exec
```

`exec` 是未来 exe.dev commands 尚未提供 typed wrapper 时的 fallback command。
