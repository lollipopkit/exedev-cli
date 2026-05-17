# exedev-ctl

[English](README.md)

`exedev-ctl` 是本 workspace 内置的 Rust exe.dev CLI。

它默认通过本机 SSH 调用 exe.dev：

```sh
ssh exe.dev <command>
```

也可以显式切换到官方 HTTPS command API：

```text
POST https://exe.dev/exec
```

## Build

```sh
cargo build -p exedev-ctl
```

通过 Cargo 运行：

```sh
cargo run -p exedev-ctl -- --help
```

或直接运行 binary：

```sh
./target/debug/exedev-ctl --help
```

## Authentication

默认 SSH 模式使用本机 `ssh exe.dev` 认证，不需要 API token。

HTTPS 模式读取 `EXE_DEV_API_KEY`：

```sh
export EXE_DEV_API_KEY="exe0...."
exedev-ctl --transport http ls
```

它也会自动加载 `.env`：

```dotenv
EXE_DEV_API_KEY=exe0....
```

shell 环境中已存在的变量优先于 `.env`。

token generation 见
[`../docs/exedev-automation.md`](../docs/exedev-automation.md)。

## Transport

默认 transport 是 SSH：

```sh
exedev-ctl ls
exedev-ctl --transport ssh ls
```

本机 SSH 不可用，或者需要在服务里用 bearer token 时，显式使用 HTTPS：

```sh
exedev-ctl --transport http ls
```

`--endpoint` 只对 HTTPS transport 生效。

## Common Commands

列出 VMs：

```sh
exedev-ctl ls
```

创建 VM：

```sh
exedev-ctl new --name p1-a-1 --image ubuntu:22.04 --no-email
```

删除 VM：

```sh
exedev-ctl rm p1-a-1
```

跳过危险操作确认：

```sh
exedev-ctl --yes rm p1-a-1
```

设置 HTTP proxy port：

```sh
exedev-ctl share port p1-a-1 8080
```

将 HTTP proxy 设为 public：

```sh
exedev-ctl share set-public p1-a-1
```

运行原始 exe.dev command：

```sh
exedev-ctl exec -- 'whoami'
```

## Output

默认 output 面向人类阅读优化。

使用 `--json` 打印 raw JSON：

```sh
exedev-ctl --json ls
```

## SSH-only Commands

官方 HTTPS `/exec` endpoint 没有 pty，也没有 stdin。以下场景即使指定
`--transport http` 也会使用本地命令：

```sh
ssh exe.dev ...
```

当前 SSH-only 场景：

- `exedev-ctl ssh ...`
- `exedev-ctl new --prompt /dev/stdin`
- `exedev-ctl new --setup-script /dev/stdin`

这些命令需要本机能够通过 SSH 访问 exe.dev。

## Coverage

CLI 覆盖 exe.dev CLI Reference 中的 top-level commands：

```text
help doc ls new rm restart rename tag stat cp resize share team whoami
ssh-key set-region integrations billing shelley browser ssh grant-support-root
exit exec
```

`exec` 是未来 exe.dev commands 尚未提供 typed wrapper 时的 fallback command。
