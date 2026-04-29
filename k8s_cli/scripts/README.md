# Node bootstrap scripts

`exedev-k8s bootstrap` uploads this directory to each VM and runs
`bootstrap-node.sh` through interactive `ssh -tt`.

The Rust CLI owns VM creation, fleet planning, kubeconfig/token retrieval,
Kubernetes metadata, and the local exe.dev API proxy. Node installation logic
lives here.

Runtime functions from `lib/exedev-cli.sh`:

- `exedev-cli-printf <level> <message>` prints progress with a stable prefix.
- `exedev-cli-ask <question> <defaultYes>` asks through the interactive TTY.
  `EXEDEV_CLI_ASSUME_YES=1` makes it auto-confirm.
- `exedev-cli-api <fnName> [params...]` calls the local Rust API proxy, which
  maps the function name and params to an exe.dev `/exec` command string.
- `exedev-cli-run <command...>` prints and runs a command.

The VM does not receive `EXE_DEV_API_KEY`; API calls are scoped to the current
SSH session through a reverse tunnel and an ephemeral proxy token.
