# exe.dev ctl

[English](README.md)

本 workspace 提供两个面向 operators 的 CLI，用于管理 exe.dev VMs 和小型
Kubernetes fleet。

## exedev-ctl

`exedev-ctl` 是 exe.dev VM 管理 CLI。它默认调用 exe.dev HTTPS command API，
对交互式命令使用 SSH fallback。

构建：

```sh
cargo build -p exedev-ctl
```

设置 API token 并列出 VMs：

```sh
export EXE_DEV_API_KEY="exe0...."
./target/debug/exedev-ctl ls
```

两个 CLI 都会自动从 `.env` 读取环境变量。shell 里已经设置的环境变量优先。

常用操作：

```sh
exedev-ctl new --name p1-a-1 --image ubuntu:22.04 --no-email
exedev-ctl share port p1-a-1 8080
exedev-ctl rm p1-a-1
```

`rm`、public share 变更、support-root grant 等危险操作默认需要确认。只有在
automation 已经审阅过 action plan 后才使用 `--yes`。

详细文档：

- [`cli/README.zh-CN.md`](cli/README.zh-CN.md)：`exedev-ctl` build、auth、output、
  fallback 和 command coverage。
- [`docs/exedev-automation.md`](docs/exedev-automation.md)：exe.dev HTTPS
  `POST /exec`、token generation 和 automation boundary。

## exedev-k8s

`exedev-k8s` 是 Kubernetes fleet 管理 CLI。它读取 `fleet.yaml`，创建或复用
exe.dev VMs，bootstrap Tailscale 和 k3s，给 nodes 打 labels/taints，并可用
`kubectl apply -f` 部署 manifests。

构建：

```sh
cargo build -p exedev-k8s
```

预览 fleet plan：

```sh
exedev-k8s plan --fleet fleet.yaml --mode new
```

bootstrap 新 k3s fleet：

```sh
export EXE_DEV_API_KEY="exe0...."
export TS_AUTHKEY="tskey-auth-..."
exedev-k8s bootstrap --fleet fleet.yaml --mode new --manifests k8s/examples
```

fleet model 从 [`fleet.example.yaml`](fleet.example.yaml) 开始。project/task
pools 会映射为确定性的 Kubernetes labels，例如 `exedev.dev/project`、
`exedev.dev/task` 和 `exedev.dev/pool`；隔离 pool 还会设置 `NoSchedule` taint。

详细文档：

- [`k8s_cli/README.zh-CN.md`](k8s_cli/README.zh-CN.md)：planning、bootstrap、deployment、
  status、destroy 和本地 secret files。
- [`docs/node-labeling.md`](docs/node-labeling.md)：Kubernetes labels、taints、
  tolerations 和 pool isolation examples。
