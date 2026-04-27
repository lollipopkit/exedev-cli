# exe.dev Kubernetes Fleet

[中文](README.zh-CN.md)

This repository manages exe.dev VMs and a small Kubernetes fleet. It keeps VM
allocation explicit, so operators can map projects and tasks to predictable node
pools before workloads are scheduled.

## Quick Start

Build the included `exedevctl` CLI:

```sh
cargo build -p exedevctl
```

Set an exe.dev API token:

```sh
export EXE_DEV_API_KEY="exe0...."
```

List current VMs:

```sh
./target/debug/exedevctl ls
```

For token generation and the underlying HTTPS API, see
[`docs/exedev-automation.md`](docs/exedev-automation.md).

## Fleet Model

[`fleet.example.yaml`](fleet.example.yaml) is the source-of-truth example for
the fleet layout. It describes:

- a control-plane pool
- project/task worker pools such as `project1/a` and `project2/b`
- VM naming prefixes such as `p1-a`
- desired node counts and default workload replicas
- optional shared or ingress spare pools
- whether each pool should be isolated with a `NoSchedule` taint

Kubernetes does not create exe.dev VMs directly. Operators provision VMs first,
join them to k3s, then apply deterministic labels and optional taints:

```sh
kubectl label node p1-a-1 exedev.dev/project=project1
kubectl label node p1-a-1 exedev.dev/task=a
kubectl label node p1-a-1 exedev.dev/pool=project1-a
kubectl taint node p1-a-1 exedev.dev/pool=project1-a:NoSchedule
```

Workloads then target the intended pool with `nodeSelector`, tolerations, or
affinity. See [`docs/node-labeling.md`](docs/node-labeling.md) for the detailed
labeling pattern.

## Common Workflows

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

Set the HTTP proxy port:

```sh
exedevctl share port p1-a-1 8080
```

Apply node labels after the VM joins the cluster:

```sh
kubectl label node p1-a-1 exedev.dev/project=project1 exedev.dev/task=a exedev.dev/pool=project1-a
```

For the full CLI surface, see [`docs/exedevctl.md`](docs/exedevctl.md).

## Safety Notes

Dangerous operations require confirmation by default. This includes deleting
VMs with `rm`, making a VM share public, granting exe.dev support root access,
and similar commands that can expose or destroy resources.

Use `--yes` only in automation that has already printed or reviewed the action
plan:

```sh
exedevctl --yes rm p1-a-1
```

## Documentation

- [`docs/exedevctl.md`](docs/exedevctl.md): CLI build, authentication, output,
  fallback behavior, and supported exe.dev commands.
- [`docs/exedev-automation.md`](docs/exedev-automation.md): HTTPS
  `POST /exec`, API token generation, command automation, and future sync
  scripts.
- [`docs/node-labeling.md`](docs/node-labeling.md): Kubernetes labels, taints,
  tolerations, and pool isolation examples.
