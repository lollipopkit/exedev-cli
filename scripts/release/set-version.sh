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

VERSION="${VERSION#v}"

# semver.org's reference grammar. The looser "digits, dots and dashes" shape this
# replaces rejected a valid tag like 1.2.3-rc.1+build.5, because build metadata can
# follow a prerelease, and accepted invalid ones like 01.2.3, which cargo refuses
# later in the release with a much less obvious error.
SEMVER_NUM='(0|[1-9][0-9]*)'
SEMVER_PRE_ID="(${SEMVER_NUM}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER_RE="^${SEMVER_NUM}\.${SEMVER_NUM}\.${SEMVER_NUM}(-${SEMVER_PRE_ID}(\.${SEMVER_PRE_ID})*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$"

if [[ ! "$VERSION" =~ $SEMVER_RE ]]; then
  echo "not a semantic version: $VERSION" >&2
  exit 1
fi

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
cleanup_staged() {
  local target
  for target in "${TARGETS[@]}"; do
    rm -f "$target.tmp" "$target.bak"
  done
}
trap cleanup_staged EXIT

for member in "${MEMBERS[@]}"; do
  manifest="$REPO_ROOT/$member/Cargo.toml"
  if [[ ! -f "$manifest" ]]; then
    echo "workspace member has no manifest: $manifest" >&2
    exit 1
  fi
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

for target in "${TARGETS[@]}"; do
  cp "$target" "$target.bak"
done
for target in "${TARGETS[@]}"; do
  mv "$target.tmp" "$target"
done

# The release build runs with --locked, which fails outright when Cargo.lock still
# carries the old member versions. Refresh it here rather than leaving the build to
# discover the mismatch.
if ! (cd "$REPO_ROOT" && cargo update --workspace --quiet); then
  for target in "${TARGETS[@]}"; do
    mv "$target.bak" "$target"
  done
  echo "cargo update failed; manifests were restored to their previous versions" >&2
  exit 1
fi

echo "Set workspace version: $VERSION"
for member in "${MEMBERS[@]}"; do
  echo "  $member/Cargo.toml"
done
for key in "${PATH_DEP_KEYS[@]}"; do
  echo "  Cargo.toml [workspace.dependencies] $key"
done
echo "  Cargo.lock"
