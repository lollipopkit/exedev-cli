# exe.dev cli

[中文](README.zh-CN.md)

This workspace provides two operator CLIs for managing exe.dev VMs and a small
Kubernetes fleet.

## exedev-ctl

`exedev-ctl` is the exe.dev VM management CLI. It calls the exe.dev HTTPS command
API, with SSH fallback for interactive commands.

Set the API token and list VMs:

```sh
# You can create one from https://exe.dev/user
export EXE_DEV_API_KEY="exe0...."
exedev-ctl ls
```

Both CLIs also load environment variables from `.env` automatically. Existing
shell environment variables take precedence.

Common operations:

```sh
exedev-ctl new --name p1-a-1 --image ubuntu:22.04 --no-email
exedev-ctl share port p1-a-1 8080
exedev-ctl rm p1-a-1
```

Dangerous operations such as `rm`, public share changes, and support-root grants
ask for confirmation by default. Use `--yes` only in reviewed automation.

Detailed documentation:

- [`cli/README.md`](cli/README.md): `exedev-ctl` build, auth, output, fallback,
  and command coverage.
- [`docs/exedev-automation.md`](docs/exedev-automation.md): exe.dev HTTPS
  `POST /exec`, token generation, and automation boundaries.

## exedev-k8s

`exedev-k8s` is the Kubernetes fleet management CLI. It reads `fleet.yaml`,
creates or reuses exe.dev VMs, bootstraps Tailscale and k3s, labels and taints
nodes, and can deploy manifests with `kubectl apply -f`.

![k8s-cli](docs/media/k8s-cli.png)

Preview a fleet plan:

```sh
exedev-k8s plan --fleet fleet.yaml --mode new
```

Bootstrap a new k3s fleet:

```sh
export EXE_DEV_API_KEY="exe0...."
# You can create one from https://login.tailscale.com/admin/machines/new-linux
export TS_AUTHKEY="tskey-auth-..."
exedev-k8s bootstrap --fleet fleet.yaml --mode new --manifests k8s/examples
```

The fleet model starts from [`fleet.example.yaml`](fleet.example.yaml). Project
and task pools become deterministic Kubernetes labels such as
`exedev.dev/project`, `exedev.dev/task`, and `exedev.dev/pool`; isolated pools
also receive a `NoSchedule` taint.

Detailed documentation:

- [`k8s_cli/README.md`](k8s_cli/README.md): planning, bootstrap, deployment,
  status, destroy, and local secret files.
- [`docs/node-labeling.md`](docs/node-labeling.md): Kubernetes labels, taints,
  tolerations, and pool isolation examples.
