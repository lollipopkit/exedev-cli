use std::{env, path::PathBuf};

pub(super) const BOOTSTRAP_NODE_SCRIPT: &str = "bootstrap-node.sh";

const SCRIPT_DIR_ENV: &str = "EXEDEV_K8S_SCRIPT_DIR";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BootstrapNodeRole {
    ControlPlanePrimary,
    ControlPlaneJoin,
    Worker,
}

impl BootstrapNodeRole {
    fn as_env(self) -> &'static str {
        match self {
            Self::ControlPlanePrimary => "control-plane-primary",
            Self::ControlPlaneJoin => "control-plane-join",
            Self::Worker => "worker",
        }
    }
}

#[derive(Debug)]
pub(super) struct BootstrapNodeRequest {
    pub(super) vm_name: String,
    pub(super) role: BootstrapNodeRole,
    pub(super) cluster_name: String,
    pub(super) ts_authkey: String,
    pub(super) k3s_token: String,
    pub(super) k3s_server_url: Option<String>,
    pub(super) assume_yes: bool,
}

impl BootstrapNodeRequest {
    pub(super) fn envs(&self) -> Vec<(String, String)> {
        let mut envs = vec![
            ("EXEDEV_VM_NAME".into(), self.vm_name.clone()),
            ("EXEDEV_NODE_ROLE".into(), self.role.as_env().into()),
            ("EXEDEV_CLUSTER_NAME".into(), self.cluster_name.clone()),
            ("TS_AUTHKEY".into(), self.ts_authkey.clone()),
            ("K3S_BOOTSTRAP_TOKEN".into(), self.k3s_token.clone()),
            (
                "EXEDEV_CLI_ASSUME_YES".into(),
                if self.assume_yes { "1" } else { "0" }.into(),
            ),
        ];
        if let Some(url) = &self.k3s_server_url {
            envs.push(("K3S_SERVER_URL".into(), url.clone()));
        }
        envs
    }
}

pub(super) fn script_dir() -> PathBuf {
    env::var_os(SCRIPT_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts"))
}
