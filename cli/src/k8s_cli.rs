use crate::DEFAULT_ENDPOINT;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "exedev-k8s")]
#[command(about = "Bootstrap and manage k3s fleets on exe.dev VMs")]
#[command(version)]
pub(crate) struct K8sCli {
    #[arg(long, global = true, default_value = DEFAULT_ENDPOINT)]
    pub(crate) endpoint: String,

    #[arg(long, global = true)]
    pub(crate) yes: bool,

    #[command(subcommand)]
    pub(crate) command: K8sCommands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ClusterMode {
    New,
    Existing,
}

#[derive(Debug, Subcommand)]
pub(crate) enum K8sCommands {
    /// Print the VM, bootstrap, and Kubernetes actions without changing state.
    Plan(PlanCmd),
    /// Create VMs, bootstrap k3s, label nodes, and optionally deploy manifests.
    Bootstrap(BootstrapCmd),
    /// Apply Kubernetes manifests with kubectl.
    Deploy(DeployCmd),
    /// Show exe.dev VM and Kubernetes node status for the fleet.
    Status(StatusCmd),
    /// Delete VMs managed by the fleet file.
    Destroy(DestroyCmd),
}

#[derive(Debug, Args)]
pub(crate) struct PlanCmd {
    #[arg(long, default_value = "fleet.yaml")]
    pub(crate) fleet: PathBuf,
    #[arg(long, value_enum, default_value_t = ClusterMode::New)]
    pub(crate) mode: ClusterMode,
}

#[derive(Debug, Args)]
pub(crate) struct BootstrapCmd {
    #[arg(long, default_value = "fleet.yaml")]
    pub(crate) fleet: PathBuf,
    #[arg(long, value_enum, default_value_t = ClusterMode::New)]
    pub(crate) mode: ClusterMode,
    #[arg(long)]
    pub(crate) manifests: Option<PathBuf>,
    #[arg(long)]
    pub(crate) kubeconfig: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct DeployCmd {
    #[arg(long)]
    pub(crate) manifests: PathBuf,
    #[arg(long)]
    pub(crate) kubeconfig: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct StatusCmd {
    #[arg(long, default_value = "fleet.yaml")]
    pub(crate) fleet: PathBuf,
    #[arg(long)]
    pub(crate) kubeconfig: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct DestroyCmd {
    #[arg(long, default_value = "fleet.yaml")]
    pub(crate) fleet: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn k8s_cli_definition_is_valid() {
        K8sCli::command().debug_assert();
    }
}
