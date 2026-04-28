# exedev-k8s Manual Test Fleets

These files are intentionally small so each scenario can be tested manually
without provisioning the full example fleet.

## Prerequisites

Build or install the CLI:

```sh
cargo build -p exedev-k8s
```

Set secrets through `.env` or the shell:

```dotenv
EXE_DEV_API_KEY=exe0....
TS_AUTHKEY=tskey-auth-...
```

For `--mode existing`, also set:

```dotenv
K3S_URL=https://100.64.0.10:6443
K3S_TOKEN=...
```

Local tools required by mutating commands:

- `ssh`
- `kubectl`

## Recommended Order

1. `01-minimal-new.yaml`
2. `02-existing-workers.yaml`
3. `03-isolated-and-shared.yaml`
4. `04-invalid-budget.yaml`

## Read-Only Checks

Every valid fleet should produce a plan:

```sh
exedev-k8s plan --fleet k8s_cli/test-fleets/01-minimal-new.yaml --mode new
exedev-k8s plan --fleet k8s_cli/test-fleets/02-existing-workers.yaml --mode existing
exedev-k8s plan --fleet k8s_cli/test-fleets/03-isolated-and-shared.yaml --mode new
```

The invalid budget fleet should fail before touching remote state:

```sh
exedev-k8s plan --fleet k8s_cli/test-fleets/04-invalid-budget.yaml --mode new
```

Expected error:

```text
fleet requests 2 worker VMs but workerVmBudget is 1
```

## Mutating Checks

Create the smallest new cluster:

```sh
exedev-k8s bootstrap --fleet k8s_cli/test-fleets/01-minimal-new.yaml --mode new
```

Check generated local secrets:

```sh
ls -l .exedev-k8s/exedev-test-minimal/
```

Expected files:

- `k3s-token`
- `kubeconfig`

Check status:

```sh
exedev-k8s status \
  --fleet k8s_cli/test-fleets/01-minimal-new.yaml \
  --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig
```

Deploy the sample workload:

```sh
kubectl --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig create namespace project1
exedev-k8s deploy \
  --manifests k8s/examples \
  --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig
kubectl --kubeconfig .exedev-k8s/exedev-test-minimal/kubeconfig -n project1 get pods -o wide
```

Destroy fleet-managed VMs when done:

```sh
exedev-k8s destroy --fleet k8s_cli/test-fleets/01-minimal-new.yaml
```

## Existing Cluster Check

Use `02-existing-workers.yaml` only after you already have a reachable k3s API
and token:

```sh
exedev-k8s bootstrap \
  --fleet k8s_cli/test-fleets/02-existing-workers.yaml \
  --mode existing \
  --kubeconfig ./kubeconfig
```

In existing mode, the control-plane node in the fleet file is only part of the
schema. `bootstrap` creates and joins worker nodes, but does not create a new
control-plane VM.
