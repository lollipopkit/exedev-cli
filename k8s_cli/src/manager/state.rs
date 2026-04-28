use super::K3S_TOKEN_ENV;
use anyhow::{Context, Result};
use rand::{RngExt, distr::Alphanumeric};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const STATE_DIR: &str = ".exedev-k8s";

pub(super) fn generated_kubeconfig_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR).join(cluster_name).join("kubeconfig")
}

pub(super) fn generated_token_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR).join(cluster_name).join("k3s-token")
}

pub(super) fn read_or_create_k3s_token(cluster_name: &str) -> Result<String> {
    let path = generated_token_path(cluster_name);
    if let Ok(token) = env::var(K3S_TOKEN_ENV) {
        if path.exists() {
            let file_token = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if file_token.trim() != token {
                write_secret_file(&path, &token)
                    .with_context(|| format!("failed to update {}", path.display()))?;
            }
        } else {
            write_secret_file(&path, &token)?;
        }
        return Ok(token);
    }
    if path.exists() {
        return fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))
            .map(|text| text.trim().to_string());
    }
    let token = random_token();
    write_secret_file(&path, &token)?;
    Ok(token)
}

pub(super) fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    Ok(())
}

pub(super) fn random_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}
