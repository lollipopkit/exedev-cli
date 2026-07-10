# Node Labels and Taints

Every VM created or managed by `exedev-k8s bootstrap` receives deterministic
labels applied automatically from `fleet.yaml`; the manual `kubectl` commands
below are only needed for out-of-band nodes or repairs (`exedev-k8s status`
reports `labels=ok taint=ok` when a node matches its spec).

## Label scheme

Control-plane nodes:

```text
exedev.dev/role=control-plane
exedev.dev/pool=control-plane
```

Control-plane nodes are not tainted by this scheme (k3s size-1 clusters
schedule workloads on the server node).

Worker nodes from a `projects.<project>.tasks.<task>` pool, where the pool
name is `<project>-<task>`:

```text
exedev.dev/project=<project>
exedev.dev/task=<task>
exedev.dev/pool=<project>-<task>
```

Spare pools from `sparePools.<name>` get `exedev.dev/pool=<name>` plus any
custom `labels` from the fleet file.

Pools with `isolated: true` (the default via `defaults.isolated`) also get a
`NoSchedule` taint so only matching workloads run there:

```text
exedev.dev/pool=<pool>:NoSchedule
```

Shared pools should set `isolated: false` so unmatched workloads can schedule
onto them.

## Manual commands

For `project1/a`:

```sh
kubectl label node p1-a-1 exedev.dev/project=project1
kubectl label node p1-a-1 exedev.dev/task=a
kubectl label node p1-a-1 exedev.dev/pool=project1-a
kubectl taint node p1-a-1 exedev.dev/pool=project1-a:NoSchedule
```

For `project2/b` with five nodes:

```sh
kubectl label node p2-b-1 exedev.dev/project=project2
kubectl label node p2-b-1 exedev.dev/task=b
kubectl label node p2-b-1 exedev.dev/pool=project2-b
kubectl taint node p2-b-1 exedev.dev/pool=project2-b:NoSchedule
```

Repeat the same labels and taint for `p2-b-2` through `p2-b-5`.

## Workload scheduling

Workloads pin to a pool with a matching selector and toleration, as in
[`../k8s/examples/project-task-deployment.yaml`](../k8s/examples/project-task-deployment.yaml):

```yaml
spec:
  nodeSelector:
    exedev.dev/project: project1
    exedev.dev/task: a
  tolerations:
    - key: exedev.dev/pool
      operator: Equal
      value: project1-a
      effect: NoSchedule
```
