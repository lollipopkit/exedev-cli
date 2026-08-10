#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

FORMULA_NAME="${FORMULA_NAME:-exedev-cli}"
FORMULA_CLASS="${FORMULA_CLASS:-ExedevCli}"
FORMULA_DESC="${FORMULA_DESC:-Unofficial CLI for exe.dev}"
FORMULA_LICENSE="${FORMULA_LICENSE:-OSL-3.0}"
REPO_SLUG="${REPO_SLUG:-lollipopkit/exedev-cli}"
TAP_REPO_PATH="${TAP_REPO_PATH:-$HOME/proj/homebrew-tap}"
TAP_FORMULA_PATH="${TAP_FORMULA_PATH:-}"
EXPLICIT_TAP_FORMULA_PATH="${TAP_FORMULA_PATH:-}"
RELEASE_TAG="${1:-${RELEASE_TAG:-}}"

# Keep in sync with the `Package release archive` step in .github/workflows/release.yml:
# the archive name and the binaries it carries are defined there, and a formula that
# guesses either one installs nothing.
ARCHIVE_PREFIX="${ARCHIVE_PREFIX:-exedev-clis}"
BINARIES=(exedev-ctl exedev-k8s)
DOCS=(README.md README.zh-CN.md fleet.example.yaml .env.example)
PLATFORMS=(macos-arm64 macos-amd64 linux-arm64 linux-amd64)

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    sha256sum "$1" | awk '{print $1}'
  fi
}

if [[ -z "$RELEASE_TAG" ]]; then
  if ! command -v gh >/dev/null 2>&1; then
    echo "RELEASE_TAG is required when gh is unavailable" >&2
    exit 1
  fi
  RELEASE_TAG="$(gh release view --repo "$REPO_SLUG" --json tagName -q .tagName)"
fi

VERSION="${RELEASE_TAG#v}"

if [[ -z "$TAP_FORMULA_PATH" && -n "$TAP_REPO_PATH" ]]; then
  # homebrew-core files its formulae under the first character of their name —
  # `Formula/e/exedev-cli.rb` — while a flat personal tap keeps them directly under
  # `Formula`. Writing to the layout the repo does not use produces a file nothing
  # installs from, and the release then reports a tap update that never reached anyone.
  FORMULA_SHARD_DIR="$TAP_REPO_PATH/Formula/${FORMULA_NAME:0:1}"
  if [[ -d "$FORMULA_SHARD_DIR" ]]; then
    TAP_FORMULA_PATH="$FORMULA_SHARD_DIR/${FORMULA_NAME}.rb"
  else
    TAP_FORMULA_PATH="$TAP_REPO_PATH/Formula/${FORMULA_NAME}.rb"
  fi
fi

if [[ -z "$TAP_FORMULA_PATH" ]]; then
  echo "TAP_REPO_PATH or TAP_FORMULA_PATH is required" >&2
  exit 1
fi

if [[ -z "$EXPLICIT_TAP_FORMULA_PATH" && -n "$TAP_REPO_PATH" && ! -d "$TAP_REPO_PATH" ]]; then
  echo "TAP_REPO_PATH does not exist: $TAP_REPO_PATH" >&2
  exit 1
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

# Parallel to PLATFORMS by index: macOS ships bash 3.2, which has no associative arrays.
SHAS=()
for platform in "${PLATFORMS[@]}"; do
  archive="${ARCHIVE_PREFIX}-${RELEASE_TAG}-${platform}.tar.gz"
  url="https://github.com/$REPO_SLUG/releases/download/$RELEASE_TAG/$archive"
  if ! curl -fsSL -o "$WORK_DIR/$archive" "$url"; then
    echo "failed to download release asset: $url" >&2
    echo "Check that the release exists and publishes every platform in PLATFORMS." >&2
    exit 1
  fi
  SHAS+=("$(sha256_of "$WORK_DIR/$archive")")
done

sha_for() {
  local target="$1" index=0
  for platform in "${PLATFORMS[@]}"; do
    if [[ "$platform" == "$target" ]]; then
      echo "${SHAS[$index]}"
      return 0
    fi
    index=$((index + 1))
  done
  echo "unknown platform: $target" >&2
  return 1
}

# The formula's `install` block names each file directly, so a renamed or dropped
# archive member fails at install time on the user's machine rather than here.
# Check the payload against the release we just downloaded instead.
tar -tzf "$WORK_DIR/${ARCHIVE_PREFIX}-${RELEASE_TAG}-macos-arm64.tar.gz" > "$WORK_DIR/members.txt"
for member in "${BINARIES[@]}" "${DOCS[@]}"; do
  if ! grep -qx "\./$member" "$WORK_DIR/members.txt"; then
    echo "release archive does not contain expected member: $member" >&2
    exit 1
  fi
done

url_for() {
  echo "https://github.com/$REPO_SLUG/releases/download/$RELEASE_TAG/${ARCHIVE_PREFIX}-${RELEASE_TAG}-${1}.tar.gz"
}

quoted_list() {
  local out=""
  for item in "$@"; do
    [[ -n "$out" ]] && out+=", "
    out+="\"$item\""
  done
  echo "$out"
}

mkdir -p "$(dirname "$TAP_FORMULA_PATH")"
cat > "$TAP_FORMULA_PATH" <<FORMULA
class $FORMULA_CLASS < Formula
  desc "$FORMULA_DESC"
  homepage "https://github.com/$REPO_SLUG"
  license "$FORMULA_LICENSE"

  livecheck do
    url :stable
    strategy :github_latest
  end

  on_macos do
    on_arm do
      url "$(url_for macos-arm64)"
      sha256 "$(sha_for macos-arm64)"
    end
    on_intel do
      url "$(url_for macos-amd64)"
      sha256 "$(sha_for macos-amd64)"
    end
  end

  on_linux do
    on_arm do
      url "$(url_for linux-arm64)"
      sha256 "$(sha_for linux-arm64)"
    end
    on_intel do
      url "$(url_for linux-amd64)"
      sha256 "$(sha_for linux-amd64)"
    end
  end

  def install
    bin.install $(quoted_list "${BINARIES[@]}")
    doc.install $(quoted_list "${DOCS[@]}")
  end

  test do
$(for binary in "${BINARIES[@]}"; do
    echo "    assert_match \"Usage: $binary\", shell_output(\"#{bin}/$binary --help\")"
  done)
  end
end
FORMULA

echo "Generated tap formula: $TAP_FORMULA_PATH"
echo "Release tag: $RELEASE_TAG"
echo "Version: $VERSION"
for platform in "${PLATFORMS[@]}"; do
  echo "SHA256 ($platform): $(sha_for $platform)"
done
