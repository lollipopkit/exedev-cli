use super::K3S_TOKEN_ENV;
use anyhow::{Context, Result};
use rand::{RngExt, distr::Alphanumeric};
use std::{
    env, fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
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

/// Writes a kubeconfig or cluster token so it is never readable by anyone else,
/// not even briefly.
///
/// Creating the file and then tightening it leaves the contents at the umask's
/// permissions in between, and a crash in that window leaves them there. Any
/// existing entry is unlinked first, so the create below applies 0600 from the
/// start and cannot follow a symlink planted at a caller-supplied `--kubeconfig`
/// path into a file that is readable elsewhere.
pub(super) fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to replace {}", path.display()));
        }
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(super) fn random_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}
