# Node Labels and Taints

Every exe.dev VM that joins the cluster should receive deterministic labels.

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

Shared pools should not be tainted by default unless they are reserved for
specific workloads.
