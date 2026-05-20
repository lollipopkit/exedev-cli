# exe.dev cli

[English](README.md)

本 workspace 提供两个面向 operators 的 CLI，用于管理 exe.dev VMs 和小型
Kubernetes fleet。

## Skill

安装 `exedev-ctl` skill：

```sh
npx skills add https://github.com/lollipopkit/exedev-cli
```

## exedev-ctl

`exedev-ctl` 是 exe.dev VM 管理 CLI。它默认使用本机 SSH，也可以通过
`--transport http` 调用 exe.dev HTTPS command API。

通过 SSH 列出 VMs：

```sh
exedev-ctl ls
```

显式使用 HTTPS API：

```sh
# 你可以从 https://exe.dev/user 创建一个
export EXE_DEV_API_KEY="exe0...."
exedev-ctl --transport http ls
```

两个 CLI 都会自动从 `.env` 读取环境变量。shell 里已经设置的环境变量优先。

常用操作：

```sh
exedev-ctl new --name p1-a-1 --image ubuntu:22.04 --no-email
exedev-ctl share port p1-a-1 8080
exedev-ctl domain add p1-a-1 app.example.com
exedev-ctl rm p1-a-1
```

`rm`、public share 变更、support-root grant 等危险操作默认需要确认。只有在
automation 已经审阅过 action plan 后才使用 `--yes`。

详细文档：

- [`cli/README.zh-CN.md`](cli/README.zh-CN.md)：`exedev-ctl` build、auth、output、
  fallback 和 command coverage。
- [`docs/exe-dev-api-reference.md`](docs/exe-dev-api-reference.md)：官方
  exe.dev API、HTTPS token、VM token 和 Login with exe 文档的本地笔记。
- [`docs/exedev-automation.md`](docs/exedev-automation.md)：exe.dev HTTPS
  `POST /exec`、token generation 和 automation boundary。

## exeuntu

[`exeuntu`](https://github.com/lollipopkit/exeuntu) 是 exe.dev 的默认 base
image。它基于 Ubuntu 24.04，面向 developer/agent 使用场景，包含 systemd，
并预装了比 minimal container image 更完整的一组 apt 工具。已发布镜像位于
`ghcr.io/lollipopkit/exeuntu`。

## Release Compatibility

Release archives 包含面向 `x86_64` 和 `aarch64` 的 Linux musl binaries，以及
面向 Intel 和 Apple Silicon 的原生 macOS binaries。Linux release binaries
使用 vendored native TLS 和 static OpenSSL 构建，运行时不依赖系统里的兼容
`libssl.so`。CI 会在 release packaging 前验证 Linux musl dist build。

## exedev-k8s

`exedev-k8s` 是 Kubernetes fleet 管理 CLI。它读取 `fleet.yaml`，创建或复用
exe.dev VMs，配置 Tailscale 和 k3s，给 nodes 打 labels/taints，并可用
`kubectl apply -f` 部署 manifests。

![k8s-cli](docs/media/k8s-cli.png)

预览 fleet plan：

```sh
exedev-k8s plan --fleet fleet.yaml --mode new
```

配置新 k3s fleet：

```sh
export EXE_DEV_API_KEY="exe0...."
# 你可以从 https://login.tailscale.com/admin/machines/new-linux 创建一个
export TS_AUTHKEY="tskey-auth-..."
exedev-k8s bootstrap --fleet fleet.yaml --mode new --manifests k8s/examples
```

fleet model 从 [`fleet.example.yaml`](fleet.example.yaml) 开始。project/task
pools 会映射为确定性的 Kubernetes labels，例如 `exedev.dev/project`、
`exedev.dev/task` 和 `exedev.dev/pool`；隔离 pool 还会设置 `NoSchedule` taint。

详细文档：

- [`k8s_cli/README.zh-CN.md`](k8s_cli/README.zh-CN.md)
- [`docs/node-labeling.md`](docs/node-labeling.md)
