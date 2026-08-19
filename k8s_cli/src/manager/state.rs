use super::K3S_TOKEN_ENV;
use anyhow::{Context, Result, bail};
use rand::{RngExt, distr::Alphanumeric};
use std::{
    env, fs,
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
};

const STATE_DIR: &str = ".exedev-k8s";

pub(super) fn generated_kubeconfig_path(cluster_name: &str) -> PathBuf {
    state_dir(cluster_name).join("kubeconfig")
}

pub(super) fn generated_token_path(cluster_name: &str) -> PathBuf {
    state_dir(cluster_name).join("k3s-token")
}

fn state_dir(cluster_name: &str) -> PathBuf {
    Path::new(STATE_DIR).join(state_dir_name(cluster_name))
}

/// TODO(remove after the next release): moves state written under the raw cluster
/// name to the sanitized directory.
///
/// Without it a cluster named `prod.example` silently starts from an empty state
/// directory, mints a fresh token, and installs a server the existing agents
/// cannot join. Only an exact rename is attempted; anything else is left alone
/// for the operator to resolve.
///
/// Called once, at bootstrap entry, rather than from the path accessors: this
/// renames a directory relative to the working directory, which is not something
/// computing a path may do.
pub(super) fn adopt_legacy_state_dir(cluster_name: &str) {
    let Some(legacy) = legacy_state_dir(cluster_name) else {
        return;
    };
    let current = state_dir(cluster_name);
    if legacy == current || current.exists() || !legacy.is_dir() {
        return;
    }
    if fs::rename(&legacy, &current).is_ok() {
        eprintln!(
            "note: moved cluster state from {} to {}",
            legacy.display(),
            current.display()
        );
    }
}

/// The directory a previous release used for this cluster, when that directory
/// is inside the state directory.
///
/// The legacy layout joined the raw cluster name, so a name carrying `..` or a
/// root resolved outside `.exedev-k8s` even then. Nothing this tool wrote is at
/// such a path, only whatever the operator keeps there, and renaming it would
/// move that directory into `.exedev-k8s` — the escape sanitizing the name exists
/// to prevent. A name that is one or more plain components stayed inside, so its
/// state is still adopted.
fn legacy_state_dir(cluster_name: &str) -> Option<PathBuf> {
    let legacy = Path::new(cluster_name);
    let mut components = legacy.components().peekable();
    // An empty name has no components at all, and `all` would call that a match.
    components.peek()?;
    components
        .all(|component| matches!(component, Component::Normal(_)))
        .then(|| Path::new(STATE_DIR).join(legacy))
}

/// Keeps a cluster name from reaching outside the state directory.
///
/// TODO(remove after the next release): a cluster whose name contains anything
/// outside `[A-Za-z0-9_-]` used its raw name as the directory before this, so its
/// kubeconfig and token are still at `.exedev-k8s/<raw name>/`. `adopt_legacy_state_dir`
/// moves them across at bootstrap entry; delete it, and this note, once no such
/// directory is expected to exist.
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
pub(super) fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
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
        // Normalized once: comparing a trimmed file against an untrimmed variable
        // rewrote the file on every run when the value carried a newline, and the
        // untrimmed value went on to the server and the agents.
        let token = token.trim().to_string();
        // An exported but empty value would otherwise become the cluster
        // credential for the server and every agent.
        if token.is_empty() {
            bail!("{K3S_TOKEN_ENV} is set but empty");
        }
        let stored = path
            .exists()
            .then(|| read_secret_file(&path))
            .transpose()?
            .map(|text| text.trim().to_string());
        if stored.as_deref() != Some(token.as_str()) {
            write_secret_file(&path, &token)
                .with_context(|| format!("failed to update {}", path.display()))?;
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
    // Secrets are written by name inside this directory, so a symlinked component
    // would place them wherever it points. Only the directories this tool creates
    // are checked; a caller-supplied --kubeconfig path is the caller's own choice
    // of destination, including its permissions.
    if !parent.starts_with(STATE_DIR) {
        return fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()));
    }
    // 0700 rather than whatever the umask allows: every directory under the state
    // directory exists to hold the cluster token and kubeconfig, and the 0600 on
    // those files is the only thing keeping them private today. The mode applies
    // to the directories this creates, `.exedev-k8s` included; one that is
    // already there is the operator's to set.
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    ensure_real_directories(parent)
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
