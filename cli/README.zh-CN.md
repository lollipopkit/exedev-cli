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

DNS 已指向 VM 后注册 custom domain：

```sh
exedev-ctl domain add p1-a-1 app.example.com
exedev-ctl domain ls p1-a-1
exedev-ctl domain rm p1-a-1 app.example.com
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
help doc ls new rm restart rename tag comment stat cp resize share domain team
pool invite whoami ssh-key set-region integrations billing shelley browser ssh
grant-support-root exit exec
```

`exec` 是未来 exe.dev commands 尚未提供 typed wrapper 时的 fallback command。它的
arguments 原样发送，不会被注入任何 flag，因此会在服务端要求确认的命令
（`team disable`、`billing credits buy`）需要自己在 raw command 里带 `--yes`。
全局 `--yes` 仍然会跳过本 CLI 自身的确认提示；全局 `--json` 也仍然生效，因为它
选择的是输出格式，不改变命令本身的行为。

有两个已文档化的 command 不提供 typed wrapper：它们都是一次性接入操作，没有
automation 价值，并且都以 argument 传递 token —— 包装成 typed wrapper 并不会让它
更安全。

```sh
exedev-ctl exec -- billing provider link aws --token="$MARKETPLACE_TOKEN"
exedev-ctl exec -- exe0-to-exe1 "$TOKEN"
```

用变量展开可以避免 token 字面量写进 shell history，但展开后的值仍然出现在本地
`exedev-ctl` 和 `ssh` 进程的 arguments 中，同一用户下的任何本地进程都能读到。
exe.dev 对这两个 command 没有提供不经过 argument 的输入方式。
