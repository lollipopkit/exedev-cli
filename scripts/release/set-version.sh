#!/usr/bin/env bash
set -euo pipefail

# Sets every workspace crate to the release version.
#
# clap's `#[command(version)]` reads CARGO_PKG_VERSION, which is baked in at compile
# time from Cargo.toml. The release tag never reaches the binary on its own, so without
# this step `exedev-ctl --version` reports whatever the crates happened to be set to
# when the tag was cut, and disagrees with the version users installed.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

VERSION="${1:-${RELEASE_TAG:-}}"

# Workspace members whose [package] version is the release version.
MEMBERS=(core cli k8s_cli)
# Keys under [workspace.dependencies] that resolve to a member by path. Their version
# requirement has to keep accepting the member, which stops holding across a minor bump.
PATH_DEP_KEYS=(exedev-core)

if [[ -z "$VERSION" ]]; then
  echo "usage: $(basename "$0") <version>" >&2
  echo "Accepts either 0.1.22 or v0.1.22; RELEASE_TAG is used when no argument is given." >&2
  exit 1
fi

# One grammar, shared with the release workflow's resolve step, which rejects a
# bad tag before any build starts.
VERSION="$("$SCRIPT_DIR/check-version.sh" "$VERSION")"

set_package_version() {
  local src="$1" dest="$2"
  awk -v ver="$VERSION" '
    /^\[/ { section = $0 }
    section == "[package]" && !replaced && /^version[[:space:]]*=/ {
      print "version = \"" ver "\""
      replaced = 1
      next
    }
    { print }
    END { exit replaced ? 0 : 1 }
  ' "$src" > "$dest"
}

set_path_dep_version() {
  local src="$1" dest="$2" key="$3"
  awk -v key="$key" -v ver="$VERSION" '
    index($0, key "=") == 1 || index($0, key " =") == 1 {
      if (sub(/version[[:space:]]*=[[:space:]]*"[^"]*"/, "version = \"" ver "\"")) replaced = 1
    }
    { print }
    END { exit replaced ? 0 : 1 }
  ' "$src" > "$dest"
}

# Every rewrite is staged next to its target and only moved into place once all of
# them have succeeded. Rewriting in a single pass left the workspace split across
# two versions whenever a later member or the lockfile refresh failed, which is
# worse than not running at all: the build then reports a version mismatch rather
# than the actual failure.
TARGETS=()
APPLIED=0
REFRESHED=0
LOCKFILE=""
LOCKFILE_CREATED=0
cleanup_staged() {
  local target
  # Nothing registered yet: `${TARGETS[@]}` on an empty array is an unbound
  # variable under `set -u`, and an early failure would exit through this.
  if [[ "${#TARGETS[@]}" -eq 0 ]]; then
    return
  fi
  # An exit between applying the manifests and refreshing the lockfile — an error,
  # a Ctrl-C, or a terminated CI step — would otherwise leave the workspace on the
  # new version with a lockfile still on the old one, and drop the backups that
  # are the only way back.
  if [[ "$APPLIED" -eq 1 && "$REFRESHED" -eq 0 ]]; then
    for target in "${TARGETS[@]}"; do
      [[ -f "$target.bak" ]] && mv "$target.bak" "$target"
    done
    if [[ "$LOCKFILE_CREATED" -eq 1 ]]; then
      rm -f "$LOCKFILE"
    fi
  fi
  for target in "${TARGETS[@]}"; do
    rm -f "$target.tmp" "$target.bak"
  done
}
trap cleanup_staged EXIT
trap 'exit 1' INT TERM

# The staging and backup names are derived from the target, so anything already
# sitting at one of them would be written through (a symlink there redirects the
# rewrite outside the workspace) and then deleted by the cleanup below.
require_free_sibling() {
  local sibling
  for sibling in "$1.tmp" "$1.next" "$1.bak"; do
    if [[ -e "$sibling" || -L "$sibling" ]]; then
      echo "refusing to run: $sibling already exists; move it aside first" >&2
      exit 1
    fi
  done
}

for member in "${MEMBERS[@]}"; do
  manifest="$REPO_ROOT/$member/Cargo.toml"
  if [[ ! -f "$manifest" ]]; then
    echo "workspace member has no manifest: $manifest" >&2
    exit 1
  fi
  require_free_sibling "$manifest"
  TARGETS+=("$manifest")
  if ! set_package_version "$manifest" "$manifest.tmp"; then
    echo "no [package] version to replace in $manifest" >&2
    exit 1
  fi
done

ROOT_MANIFEST="$REPO_ROOT/Cargo.toml"
if [[ ! -f "$ROOT_MANIFEST" ]]; then
  echo "workspace has no root manifest: $ROOT_MANIFEST" >&2
  exit 1
fi
require_free_sibling "$ROOT_MANIFEST"
TARGETS+=("$ROOT_MANIFEST")
cp "$ROOT_MANIFEST" "$ROOT_MANIFEST.tmp"
for key in "${PATH_DEP_KEYS[@]}"; do
  # Each key edits the staged copy, so several of them accumulate in one file.
  if ! set_path_dep_version "$ROOT_MANIFEST.tmp" "$ROOT_MANIFEST.next" "$key"; then
    rm -f "$ROOT_MANIFEST.next"
    echo "no versioned '$key' entry to replace in $ROOT_MANIFEST" >&2
    exit 1
  fi
  mv "$ROOT_MANIFEST.next" "$ROOT_MANIFEST.tmp"
done

# Cargo.lock is backed up but never staged: `cargo update` writes it below, and a
# failure or interrupt there would otherwise leave a refreshed lockfile beside
# restored manifests.
LOCKFILE="$REPO_ROOT/Cargo.lock"
LOCKFILE_CREATED=0
if [[ -f "$LOCKFILE" ]]; then
  require_free_sibling "$LOCKFILE"
  TARGETS+=("$LOCKFILE")
else
  # Nothing to restore it to: a workspace that had no lockfile must not keep the
  # one `cargo update` writes if the run does not finish.
  LOCKFILE_CREATED=1
fi

for target in "${TARGETS[@]}"; do
  cp "$target" "$target.bak"
done
# Set before the first move, not after the last: an interrupt or a failing mv
# partway through leaves some manifests new and some old, which is exactly the
# state the restore exists for.
APPLIED=1
for target in "${TARGETS[@]}"; do
  [[ -f "$target.tmp" ]] && mv "$target.tmp" "$target"
done

# The release build runs with --locked, which fails outright when Cargo.lock still
# carries the old member versions. Refresh it here rather than leaving the build to
# discover the mismatch. `--workspace` re-resolves only the workspace members, so
# no third-party dependency selection changes and the tag still builds from its
# reviewed lockfile.
if ! (cd "$REPO_ROOT" && cargo update --workspace --quiet); then
  echo "cargo update failed; manifests are being restored to their previous versions" >&2
  exit 1
fi
REFRESHED=1

echo "Set workspace version: $VERSION"
for member in "${MEMBERS[@]}"; do
  echo "  $member/Cargo.toml"
done
for key in "${PATH_DEP_KEYS[@]}"; do
  echo "  Cargo.toml [workspace.dependencies] $key"
done
echo "  Cargo.lock"
