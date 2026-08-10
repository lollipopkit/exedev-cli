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
  local file="$1"
  awk -v ver="$VERSION" '
    /^\[/ { section = $0 }
    section == "[package]" && !replaced && /^version[[:space:]]*=/ {
      print "version = \"" ver "\""
      replaced = 1
      next
    }
    { print }
    END { exit replaced ? 0 : 1 }
  ' "$file" > "$file.tmp"
}

set_path_dep_version() {
  local file="$1" key="$2"
  awk -v key="$key" -v ver="$VERSION" '
    index($0, key "=") == 1 || index($0, key " =") == 1 {
      if (sub(/version[[:space:]]*=[[:space:]]*"[^"]*"/, "version = \"" ver "\"")) replaced = 1
    }
    { print }
    END { exit replaced ? 0 : 1 }
  ' "$file" > "$file.tmp"
}

for member in "${MEMBERS[@]}"; do
  manifest="$REPO_ROOT/$member/Cargo.toml"
  if [[ ! -f "$manifest" ]]; then
    echo "workspace member has no manifest: $manifest" >&2
    exit 1
  fi
  if ! set_package_version "$manifest"; then
    rm -f "$manifest.tmp"
    echo "no [package] version to replace in $manifest" >&2
    exit 1
  fi
  mv "$manifest.tmp" "$manifest"
done

for key in "${PATH_DEP_KEYS[@]}"; do
  if ! set_path_dep_version "$REPO_ROOT/Cargo.toml" "$key"; then
    rm -f "$REPO_ROOT/Cargo.toml.tmp"
    echo "no versioned '$key' entry to replace in $REPO_ROOT/Cargo.toml" >&2
    exit 1
  fi
  mv "$REPO_ROOT/Cargo.toml.tmp" "$REPO_ROOT/Cargo.toml"
done

# The release build runs with --locked, which fails outright when Cargo.lock still
# carries the old member versions. Refresh it here rather than leaving the build to
# discover the mismatch.
(cd "$REPO_ROOT" && cargo update --workspace --quiet)

echo "Set workspace version: $VERSION"
for member in "${MEMBERS[@]}"; do
  echo "  $member/Cargo.toml"
done
for key in "${PATH_DEP_KEYS[@]}"; do
  echo "  Cargo.toml [workspace.dependencies] $key"
done
echo "  Cargo.lock"
