use super::K3S_TOKEN_ENV;
use anyhow::{Context, Result, bail};
use rand::{RngExt, distr::Alphanumeric};
use std::{
    env, fs,
    io::Write,
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
            let file_token = read_regular_file(&path)?;
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
        return read_regular_file(&path).map(|text| text.trim().to_string());
    }
    let token = random_token();
    write_secret_file(&path, &token)?;
    Ok(token)
}

/// Writes a kubeconfig or cluster token so it is never readable by anyone else,
/// not even briefly, and never observable half-written.
///
/// The contents go to a staging file in the same directory, created 0600 so they
/// are never present at the umask's permissions, and only a completed write is
/// renamed over `path`. Writing into the destination directly would leave a
/// truncated token or kubeconfig behind if the write failed partway, and the next
/// run reads whatever is at the path without being able to tell it is a fragment.
/// The rename also replaces a symlink rather than following one planted at a
/// caller-supplied `--kubeconfig` path.
pub(super) fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let staged = staging_path(path);
    // A staged file from a crashed run with this pid would fail the create below.
    let _ = fs::remove_file(&staged);
    let write = || -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staged)
            .with_context(|| format!("failed to create {}", staged.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("failed to write {}", staged.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to flush {}", staged.display()))?;
        Ok(())
    };
    if let Err(err) = write() {
        let _ = fs::remove_file(&staged);
        return Err(err);
    }
    if let Err(err) = fs::rename(&staged, path) {
        let _ = fs::remove_file(&staged);
        return Err(err).with_context(|| format!("failed to replace {}", path.display()));
    }
    Ok(())
}

fn staging_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "secret".to_string());
    let staged = format!(".{name}.{}.tmp", std::process::id());
    match path.parent() {
        Some(parent) => parent.join(staged),
        None => PathBuf::from(staged),
    }
}

/// Reads a file that must be a real file this tool wrote.
///
/// `fs::read_to_string` follows symlinks, so an entry swapped for a link to
/// another readable file would have that file's contents adopted as the cluster
/// token.
pub(super) fn read_regular_file(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.is_file() {
        bail!(
            "{} is not a regular file; remove it and rerun",
            path.display()
        );
    }
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub(super) fn random_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}
