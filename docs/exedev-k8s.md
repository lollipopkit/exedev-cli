# exedev-k8s

`exedev-k8s` is the Kubernetes fleet CLI for this repository. It reads
`fleet.yaml`, creates the required exe.dev VMs, bootstraps Tailscale and k3s,
applies Kubernetes node labels and taints, and can deploy manifests with
`kubectl apply -f`.

Build it with the existing Cargo package:

```sh
cargo build -p exedevctl
```

Run the binary directly:

```sh
./target/debug/exedev-k8s --help
```

## Requirements

Local tools:

- `ssh`
- `kubectl`

Environment variables:

- `EXE_DEV_API_KEY`: exe.dev HTTPS API token.
- `TS_AUTHKEY`: Tailscale auth key, required when `defaults.network` is
  `tailscale`.
- `K3S_TOKEN`: optional for `--mode new`; generated under `.exedev-k8s/` when
  omitted. Required for `--mode existing`.
- `K3S_URL`: required for `--mode existing`, for example
  `https://100.64.0.10:6443`.

Secrets should stay in environment variables or `.exedev-k8s/`; do not commit
them into `fleet.yaml`.

## Fleet File

The CLI defaults to `fleet.yaml`. Use `fleet.example.yaml` as the starting
point:

```yaml
defaults:
  network: tailscale
  kubernetes: k3s
  image: ubuntu:22.04
  isolated: true
```

Version 1 supports one k3s control-plane node:

```yaml
cluster:
  controlPlane:
    nodes: 1
```

Project/task pools become worker nodes. For `project1/a`, the CLI applies:

```text
exedev.dev/project=project1
exedev.dev/task=a
exedev.dev/pool=project1-a
```

If the pool is isolated, it also applies:

```text
exedev.dev/pool=project1-a:NoSchedule
```

## Commands

Preview the bootstrap plan:

```sh
exedev-k8s plan --fleet fleet.yaml --mode new
```

Create a new k3s cluster:

```sh
export EXE_DEV_API_KEY="exe0...."
export TS_AUTHKEY="tskey-auth-..."
exedev-k8s bootstrap --fleet fleet.yaml --mode new
```

Join workers to an existing k3s cluster:

```sh
export EXE_DEV_API_KEY="exe0...."
export TS_AUTHKEY="tskey-auth-..."
export K3S_URL="https://100.64.0.10:6443"
export K3S_TOKEN="..."
exedev-k8s bootstrap --fleet fleet.yaml --mode existing --kubeconfig ./kubeconfig
```

Bootstrap and deploy manifests:

```sh
exedev-k8s bootstrap --fleet fleet.yaml --mode new --manifests k8s/examples
```

Deploy manifests only:

```sh
exedev-k8s deploy --manifests k8s/examples --kubeconfig .exedev-k8s/exedev-main/kubeconfig
```

Show VM and Kubernetes node status:

```sh
exedev-k8s status --fleet fleet.yaml --kubeconfig .exedev-k8s/exedev-main/kubeconfig
```

Delete fleet-managed VMs:

```sh
exedev-k8s destroy --fleet fleet.yaml --yes
```

`destroy` is separate from `bootstrap`; bootstrap never deletes extra VMs.

## Safety

`plan` is read-only. `bootstrap` and `destroy` print the planned actions and ask
for confirmation unless `--yes` is passed.

`bootstrap` creates missing VMs, installs Tailscale and k3s through local
`ssh exe.dev ssh <vm> ...`, applies labels and taints with `kubectl`, and
optionally runs `kubectl apply -f <dir>`.

The generated kubeconfig and k3s token for new clusters are stored under:

```text
.exedev-k8s/<cluster-name>/
```
