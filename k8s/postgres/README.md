# PostgreSQL fleet manifests

These manifests target `fleet.pgsql.yaml`: three PostgreSQL data workers,
three k3s control-plane nodes, and one Tailnet access worker.

Before applying `k8s/postgres`, create the backup secret:

```sh
kubectl --kubeconfig .exedev-k8s/exedev-pgsql/kubeconfig apply -f k8s/postgres/namespace.yaml
kubectl --kubeconfig .exedev-k8s/exedev-pgsql/kubeconfig -n database create secret generic pg-main-backup-s3 \
  --from-literal=ACCESS_KEY_ID="$AWS_ACCESS_KEY_ID" \
  --from-literal=ACCESS_SECRET_KEY="$AWS_SECRET_ACCESS_KEY" \
  --dry-run=client -o yaml | kubectl --kubeconfig .exedev-k8s/exedev-pgsql/kubeconfig apply -f -
```

Install the Tailscale Kubernetes Operator before expecting the `pg-main-rw`
LoadBalancer service to appear in the tailnet. The operator requires OAuth
client credentials with `Devices Core`, `Auth Keys`, and `Services` write
scopes. The static service template uses:

```yaml
loadBalancerClass: tailscale
tailscale.com/hostname: pg-main-rw
```

Deploy:

```sh
exedev-k8s bootstrap --fleet fleet.pgsql.yaml --mode new --manifests k8s/postgres
```

If CRDs are not fully established on the first apply, rerun:

```sh
exedev-k8s deploy --manifests k8s/postgres --kubeconfig .exedev-k8s/exedev-pgsql/kubeconfig
```
