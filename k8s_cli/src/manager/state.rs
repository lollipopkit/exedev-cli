use super::K3S_TOKEN_ENV;
use anyhow::{Context, Result, bail};
use rand::{RngExt, distr::Alphanumeric};
use std::{
    env, fs,
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const STATE_DIR: &str = ".exedev-k8s";

pub(super) fn generated_kubeconfig_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR)
        .join(state_dir_name(cluster_name))
        .join("kubeconfig")
}

pub(super) fn generated_token_path(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR)
        .join(state_dir_name(cluster_name))
        .join("k3s-token")
}

/// Keeps a cluster name from reaching outside the state directory.
///
/// The name comes from fleet.yaml, which only requires it to be non-empty, so
/// `../../elsewhere` would otherwise place the token and kubeconfig outside
/// `.exedev-k8s`. Anything that is not a plain name component is replaced.
fn state_dir_name(cluster_name: &str) -> String {
    let safe = !cluster_name.is_empty()
        && cluster_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if safe {
        return cluster_name.to_string();
    }
    // Two names that sanitize alike — `a/b` and `a_b` — would otherwise share one
    // directory and overwrite each other's token. The digest of the original name
    // keeps them apart while the sanitized part keeps the directory recognizable.
    let sanitized = cluster_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{sanitized}-{:016x}", fnv1a(cluster_name.as_bytes()))
}

/// FNV-1a, spelled out so the directory a cluster uses never changes with the
/// toolchain the way `DefaultHasher` would.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

pub(super) fn read_or_create_k3s_token(cluster_name: &str) -> Result<String> {
    let path = generated_token_path(cluster_name);
    // Before any read follows it: a symlinked `.exedev-k8s` or cluster directory
    // would otherwise have an outside file adopted as this cluster's credential.
    if let Some(parent) = path.parent()
        && parent.exists()
    {
        ensure_real_directories(parent)?;
    }
    if let Ok(token) = env::var(K3S_TOKEN_ENV) {
        // An exported but empty value would otherwise become the cluster
        // credential for the server and every agent.
        if token.trim().is_empty() {
            bail!("{K3S_TOKEN_ENV} is set but empty");
        }
        if path.exists() {
            let file_token = read_secret_file(&path)?;
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
        let token = read_secret_file(&path).map(|text| text.trim().to_string())?;
        // An empty file is not a token. Returning it would hand the server and
        // every agent a blank credential, the same way an empty K3S_TOKEN would.
        if token.is_empty() {
            bail!(
                "{} is empty; delete it to generate a new cluster token",
                path.display()
            );
        }
        return Ok(token);
    }
    create_k3s_token(&path)
}

/// Generates the cluster token, or adopts the one another run created first.
///
/// Writing it outright would let two bootstraps of the same cluster each
/// generate a token, clobber the other, and hand the server and the agents
/// different credentials. The staged file is linked into place instead, which
/// fails rather than replaces when the name is already taken, so whoever loses
/// the race reads the winner's token.
pub(super) fn create_k3s_token(path: &Path) -> Result<String> {
    prepare_secret_parent(path)?;
    let token = random_token();
    let staged = stage_secret(path, &token)?;
    match fs::hard_link(&staged, path) {
        Ok(()) => {
            let sync = sync_parent_dir(path);
            let _ = fs::remove_file(&staged);
            sync?;
            Ok(token)
        }
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&staged);
            read_secret_file(path).map(|text| text.trim().to_string())
        }
        Err(err) => {
            let _ = fs::remove_file(&staged);
            Err(err).with_context(|| format!("failed to create {}", path.display()))
        }
    }
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
    prepare_secret_parent(path)?;
    let staged = stage_secret(path, contents)?;
    if let Err(err) = fs::rename(&staged, path) {
        let _ = fs::remove_file(&staged);
        return Err(err).with_context(|| format!("failed to replace {}", path.display()));
    }
    sync_parent_dir(path)
}

/// Flushes the directory entry a rename or link just created.
///
/// `sync_all` on the staged file persists its contents, not the name it was
/// published under, so a crash could otherwise leave a bootstrapped cluster whose
/// token this tool no longer has.
fn sync_parent_dir(path: &Path) -> Result<()> {
    let parent = match path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::from("."),
    };
    let dir = fs::File::open(&parent)
        .with_context(|| format!("failed to open {} to flush it", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to flush {}", parent.display()))
}

fn prepare_secret_parent(path: &Path) -> Result<()> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(());
    };
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    // Secrets are written by name inside this directory, so a symlinked component
    // would place them wherever it points. Only the directories this tool creates
    // are checked; a caller-supplied --kubeconfig path is the caller's own choice
    // of destination.
    if parent.starts_with(STATE_DIR) {
        ensure_real_directories(parent)?;
    }
    Ok(())
}

/// Writes `contents` to a fresh 0600 file beside `path` and returns its path.
fn stage_secret(path: &Path, contents: &str) -> Result<PathBuf> {
    let staged = staging_path(path);
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
    Ok(staged)
}

/// A staging name that cannot be guessed ahead of the write.
///
/// The name is random rather than derived from the pid so nothing can be waiting
/// at it. Combined with `create_new`, which refuses an existing entry of any kind
/// including a symlink, the staged file is always one this process just made.
fn staging_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "secret".to_string());
    let staged = format!(".{name}.{}.tmp", random_suffix());
    match path.parent() {
        Some(parent) => parent.join(staged),
        None => PathBuf::from(staged),
    }
}

/// Reads a secret this tool wrote, tightening it if a previous run or a restored
/// backup left it readable by others.
///
/// The permissions are changed through the handle the contents are read from.
/// `fs::set_permissions` takes a path and follows symlinks, so doing it by name
/// could chmod whatever an entry swapped in the meantime points at.
pub(super) fn read_secret_file(path: &Path) -> Result<String> {
    let (contents, file) = open_regular_file(path)?;
    let mode = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o077 != 0 {
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to restrict permissions on {}", path.display()))?;
    }
    Ok(contents)
}

fn ensure_real_directories(dir: &Path) -> Result<()> {
    let mut walked = PathBuf::new();
    for component in dir.components() {
        walked.push(component);
        let metadata = fs::symlink_metadata(&walked)
            .with_context(|| format!("failed to inspect {}", walked.display()))?;
        if !metadata.is_dir() {
            bail!(
                "{} is not a real directory; remove it and rerun",
                walked.display()
            );
        }
    }
    Ok(())
}

fn random_suffix() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

fn open_regular_file(path: &Path) -> Result<(String, fs::File)> {
    let before = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !before.is_file() {
        bail!(
            "{} is not a regular file; remove it and rerun",
            path.display()
        );
    }
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let opened = file
        .metadata()
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if opened.dev() != before.dev() || opened.ino() != before.ino() {
        bail!("{} changed while it was being read", path.display());
    }
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok((contents, file))
}

pub(super) fn random_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}
