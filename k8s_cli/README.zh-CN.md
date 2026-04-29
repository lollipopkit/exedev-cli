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
- `scp`
- `kubectl`

环境变量：

- `EXE_DEV_API_KEY`: exe.dev HTTPS API token。
- `TS_AUTHKEY`: Tailscale auth key，当 `defaults.network` 为 `tailscale` 时必需。
- `K3S_TOKEN`: `--mode new` 时可选；缺省会生成到 `.exedev-k8s/` 下。
  `--mode existing` 时必需。
- `K3S_URL`: `--mode existing` 时必需，例如 `https://100.64.0.10:6443`。

建议 k3s nodes 使用 exe.dev 的 `exeuntu` image。`ubuntu:22.04` 这类 generic
minimal image 可能缺少 `sudo`、`curl` 或 systemd/openrc 这类 process
supervisor。

如果启用了 Tailnet Lock，且新 VM 处于 locked out 状态，`bootstrap` 会在安装
k3s 前暂停并打印 `tailscale lock sign ...` 命令。请在 trusted signing node 上
执行该命令，签名完成后重新运行 `exedev-k8s bootstrap`。

CLI 会自动加载 `.env`，且不会覆盖 shell 环境中已经存在的变量：

```dotenv
EXE_DEV_API_KEY=exe0....
TS_AUTHKEY=tskey-auth-...
K3S_URL=https://100.64.0.10:6443
K3S_TOKEN=...
```

secrets 应保存在环境变量、`.env` 或 `.exedev-k8s/` 中；不要提交到 `fleet.yaml`。

## Fleet File

CLI 默认读取 `fleet.yaml`。以
[`../fleet.example.yaml`](../fleet.example.yaml) 作为起点：

```yaml
defaults:
  network: tailscale
  kubernetes: k3s
  image: exeuntu
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
exedev-k8s destroy --fleet fleet.yaml
```

`destroy` 与 `bootstrap` 分离；`bootstrap` 不会删除额外 VMs。

## Manual Test Fleets

小型测试 fleet 放在 [`test-fleets/`](test-fleets/)。推荐先跑
`01-minimal-new.yaml` 创建基础 cluster，再用 `02-existing-workers.yaml` 将额外
workers 加入这个 cluster。

创建 fleet 1：

```sh
exedev-k8s bootstrap \
  --fleet k8s_cli/test-fleets/01-minimal-new.yaml \
  --mode new
```

fleet 1 生成的 kubeconfig 和 token 会保存到：

```text
.exedev-k8s/exedev-test-minimal/kubeconfig
.exedev-k8s/exedev-test-minimal/k3s-token
```

在 fleet 1 基础上测试 fleet 2：

```sh
export K3S_URL="$(kubectl --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig config view --minify -o jsonpath='{.clusters[0].cluster.server}')"
export K3S_TOKEN="$(cat .exedev-k8s/exedev-test-minimal/k3s-token)"

exedev-k8s bootstrap \
  --fleet k8s_cli/test-fleets/02-existing-workers.yaml \
  --mode existing \
  --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig
```

`--mode existing` 一定要显式传入目标 cluster 的 `--kubeconfig`，否则 `kubectl`
会使用默认配置，可能出现 `/readyz` 返回 `NotFound`，或者 labels/taints 未应用到
正确 cluster。

如果 Tailnet Lock 让 bootstrap 暂停，请在 trusted signing node 上执行输出中的
`tailscale lock sign ...` 命令，然后原样重跑上面的 `bootstrap`。已创建的 VMs
会被复用。

检查 fleet 2 是否已加入 fleet 1：

```sh
exedev-k8s status \
  --fleet k8s_cli/test-fleets/02-existing-workers.yaml \
  --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig

kubectl --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig get nodes -o wide
```

预期 `test-ex-p1-a-1` 和 `test-ex-p2-b-1` 是 `Ready`，并且 `status` 显示
`labels=ok taint=ok`。

## Safety

`plan` 是 read-only。`bootstrap` 会打印 planned actions，并在没有传入 `--yes`
时请求确认。`destroy` 始终需要确认，即使传入全局 `--yes`。

`bootstrap` 会创建缺失 VMs，通过 `scp` 上传 `k8s_cli/scripts/`，再用交互式
`ssh -tt` 执行 node bootstrap Bash 脚本；随后用 `kubectl` 应用
labels/taints，并可选运行 `kubectl apply -f <dir>`。如果目录内有
`kustomization.yaml`，会改用 `kubectl apply -k <dir>`。

新 cluster 生成的 kubeconfig 和 k3s token 会保存到：

```text
.exedev-k8s/<cluster-name>/
```
