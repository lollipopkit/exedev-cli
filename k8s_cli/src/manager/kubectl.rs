use super::process::{capture_command, run_command};
use anyhow::Result;
use std::{path::Path, process::Stdio};

pub(super) const KUBECTL_DEFAULT_REQUEST_TIMEOUT: &str = "30s";

pub(super) const KUBECTL_PROBE_REQUEST_TIMEOUT: &str = "8s";

pub(super) async fn kubectl_apply(kubeconfig: Option<&Path>, manifests: &Path) -> Result<()> {
    let apply_flag = if manifests.is_dir() && manifests.join("kustomization.yaml").exists() {
        "-k"
    } else {
        "-f"
    };
    kubectl_run_owned(
        kubeconfig,
        vec![
            "apply".into(),
            apply_flag.into(),
            manifests.display().to_string(),
        ],
    )
    .await
}

pub(super) async fn kubectl_run_owned(kubeconfig: Option<&Path>, args: Vec<String>) -> Result<()> {
    let mut words = kubeconfig_args(kubeconfig, KUBECTL_DEFAULT_REQUEST_TIMEOUT);
    words.extend(args);
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    run_command("kubectl", &refs, Stdio::inherit()).await
}

pub(super) async fn kubectl_capture(kubeconfig: Option<&Path>, args: &[&str]) -> Result<String> {
    kubectl_capture_with_timeout(kubeconfig, args, KUBECTL_DEFAULT_REQUEST_TIMEOUT).await
}

pub(super) async fn kubectl_capture_with_timeout(
    kubeconfig: Option<&Path>,
    args: &[&str],
    request_timeout: &str,
) -> Result<String> {
    let mut words = kubeconfig_args(kubeconfig, request_timeout);
    words.extend(args.iter().map(|arg| (*arg).to_string()));
    let refs = words.iter().map(String::as_str).collect::<Vec<_>>();
    capture_command("kubectl", &refs).await
}

pub(super) fn kubeconfig_args(kubeconfig: Option<&Path>, request_timeout: &str) -> Vec<String> {
    let mut args = kubeconfig
        .map(|path| vec!["--kubeconfig".into(), path.display().to_string()])
        .unwrap_or_default();
    args.push(format!("--request-timeout={request_timeout}"));
    args
}
