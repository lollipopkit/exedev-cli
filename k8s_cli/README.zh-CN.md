# exedev-k8s

[English](README.md)

`exedev-k8s` 是本 workspace 的 Kubernetes fleet CLI。它读取 `fleet.yaml`，
创建所需的 exe.dev VMs，bootstrap Tailscale 和 k3s，应用 Kubernetes node
labels/taints，并可用 `kubectl apply -f` 部署 manifests。

使用对应 Cargo package 构建：

```sh
cargo build -p exedev-k8s
```

直接运行 binary：

```sh
./target/debug/exedev-k8s --help
```

## Requirements

本地工具：

- `ssh`
- `kubectl`

环境变量：

- `EXE_DEV_API_KEY`: exe.dev HTTPS API token。
- `TS_AUTHKEY`: Tailscale auth key，当 `defaults.network` 为 `tailscale` 时必需。
- `K3S_TOKEN`: `--mode new` 时可选；缺省会生成到 `.exedev-k8s/` 下。
  `--mode existing` 时必需。
- `K3S_URL`: `--mode existing` 时必需，例如 `https://100.64.0.10:6443`。

secrets 应保存在环境变量或 `.exedev-k8s/` 中；不要提交到 `fleet.yaml`。

## Fleet File

CLI 默认读取 `fleet.yaml`。以
[`../fleet.example.yaml`](../fleet.example.yaml) 作为起点：

```yaml
defaults:
  network: tailscale
  kubernetes: k3s
  image: ubuntu:22.04
  isolated: true
```

Version 1 支持一个 k3s control-plane node：

```yaml
cluster:
  controlPlane:
    nodes: 1
```

project/task pools 会成为 worker nodes。对于 `project1/a`，CLI 会应用：

```text
exedev.dev/project=project1
exedev.dev/task=a
exedev.dev/pool=project1-a
```

如果 pool 被隔离，还会应用：

```text
exedev.dev/pool=project1-a:NoSchedule
```

workload 侧 scheduling pattern 见
[`../docs/node-labeling.md`](../docs/node-labeling.md)。

## Commands

预览 bootstrap plan：

```sh
exedev-k8s plan --fleet fleet.yaml --mode new
```

创建新的 k3s cluster：

```sh
export EXE_DEV_API_KEY="exe0...."
export TS_AUTHKEY="tskey-auth-..."
exedev-k8s bootstrap --fleet fleet.yaml --mode new
```

将 workers 加入已有 k3s cluster：

```sh
export EXE_DEV_API_KEY="exe0...."
export TS_AUTHKEY="tskey-auth-..."
export K3S_URL="https://100.64.0.10:6443"
export K3S_TOKEN="..."
exedev-k8s bootstrap --fleet fleet.yaml --mode existing --kubeconfig ./kubeconfig
```

bootstrap 并部署 manifests：

```sh
exedev-k8s bootstrap --fleet fleet.yaml --mode new --manifests k8s/examples
```

只部署 manifests：

```sh
exedev-k8s deploy --manifests k8s/examples --kubeconfig .exedev-k8s/exedev-main/kubeconfig
```

查看 VM 和 Kubernetes node status：

```sh
exedev-k8s status --fleet fleet.yaml --kubeconfig .exedev-k8s/exedev-main/kubeconfig
```

删除 fleet 管理的 VMs：

```sh
exedev-k8s destroy --fleet fleet.yaml --yes
```

`destroy` 与 `bootstrap` 分离；`bootstrap` 不会删除额外 VMs。

## Safety

`plan` 是 read-only。`bootstrap` 和 `destroy` 会打印 planned actions，并在没有
传入 `--yes` 时请求确认。

`bootstrap` 会创建缺失 VMs，通过本地 `ssh exe.dev ssh <vm> ...` 安装 Tailscale
和 k3s，用 `kubectl` 应用 labels/taints，并可选运行 `kubectl apply -f <dir>`。

新 cluster 生成的 kubeconfig 和 k3s token 会保存到：

```text
.exedev-k8s/<cluster-name>/
```
