# exe.dev Kubernetes Fleet

[English](README.md)

本仓库用于管理 exe.dev VMs 和一个小型 Kubernetes fleet。它把 VM 分配显式化，
让 operators 可以先把 project/task 映射到可预测的 node pools，再调度 workloads。

## Quick Start

构建仓库内置的 `exedevctl` CLI：

```sh
cargo build -p exedevctl
```

设置 exe.dev API token：

```sh
export EXE_DEV_API_KEY="exe0...."
```

列出现有 VMs：

```sh
./target/debug/exedevctl ls
```

token 生成方式和底层 HTTPS API 见
[`docs/exedev-automation.md`](docs/exedev-automation.md)。

## Fleet Model

[`fleet.example.yaml`](fleet.example.yaml) 是 fleet layout 的 source of truth 示例。
它描述：

- control-plane pool
- `project1/a`、`project2/b` 等 project/task worker pools
- `p1-a` 等 VM naming prefixes
- desired node counts 和 default workload replicas
- 可选的 shared 或 ingress spare pools
- 每个 pool 是否用 `NoSchedule` taint 隔离

Kubernetes 不直接创建 exe.dev VMs。operators 先 provision VMs，把它们加入 k3s，
再应用确定性的 labels 和可选 taints：

```sh
kubectl label node p1-a-1 exedev.dev/project=project1
kubectl label node p1-a-1 exedev.dev/task=a
kubectl label node p1-a-1 exedev.dev/pool=project1-a
kubectl taint node p1-a-1 exedev.dev/pool=project1-a:NoSchedule
```

workloads 再通过 `nodeSelector`、tolerations 或 affinity 选择目标 pool。详细 label
模式见 [`docs/node-labeling.md`](docs/node-labeling.md)。

## Common Workflows

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

设置 HTTP proxy port：

```sh
exedevctl share port p1-a-1 8080
```

VM 加入 cluster 后应用 node labels：

```sh
kubectl label node p1-a-1 exedev.dev/project=project1 exedev.dev/task=a exedev.dev/pool=project1-a
```

完整 CLI 说明见 [`docs/exedevctl.md`](docs/exedevctl.md)。

## Safety Notes

危险操作默认需要确认，包括用 `rm` 删除 VMs、把 VM share 设为 public、授予
exe.dev support root access，以及类似会暴露或销毁资源的命令。

只有在 automation 已经打印或审阅 action plan 后才使用 `--yes`：

```sh
exedevctl --yes rm p1-a-1
```

## Documentation

- [`docs/exedevctl.md`](docs/exedevctl.md)：CLI build、authentication、output、
  fallback behavior 和支持的 exe.dev commands。
- [`docs/exedev-automation.md`](docs/exedev-automation.md)：HTTPS `POST /exec`、
  API token generation、command automation 和未来 sync scripts。
- [`docs/node-labeling.md`](docs/node-labeling.md)：Kubernetes labels、taints、
  tolerations 和 pool isolation examples。
