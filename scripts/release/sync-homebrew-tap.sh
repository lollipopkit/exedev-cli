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
DOCS=(README.md README.zh-CN.md LICENSE fleet.example.yaml .env.example)
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

# Every value below is interpolated into download URLs, local file paths, and
# double-quoted Ruby strings in the formula. Validate them here rather than
# escaping at each use: a stray quote, newline, or slash otherwise produces a
# formula that generation reports as a success and Homebrew cannot parse.
SEMVER_NUM='(0|[1-9][0-9]*)'
SEMVER_PRE_ID="(${SEMVER_NUM}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER_RE="^${SEMVER_NUM}\.${SEMVER_NUM}\.${SEMVER_NUM}(-${SEMVER_PRE_ID}(\.${SEMVER_PRE_ID})*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$"

if [[ ! "$VERSION" =~ $SEMVER_RE ]]; then
  echo "release tag is not a semantic version: $RELEASE_TAG" >&2
  echo "Expected something like v0.1.11 or 1.2.3-rc.1+build.5." >&2
  exit 1
fi

if [[ ! "$REPO_SLUG" =~ ^[0-9A-Za-z._-]+/[0-9A-Za-z._-]+$ ]]; then
  echo "REPO_SLUG is not an owner/repo slug: $REPO_SLUG" >&2
  exit 1
fi

if [[ ! "$FORMULA_NAME" =~ ^[0-9A-Za-z._-]+$ ]]; then
  echo "FORMULA_NAME is not a formula name: $FORMULA_NAME" >&2
  exit 1
fi

if [[ ! "$ARCHIVE_PREFIX" =~ ^[0-9A-Za-z._-]+$ ]]; then
  echo "ARCHIVE_PREFIX is not an archive name prefix: $ARCHIVE_PREFIX" >&2
  exit 1
fi

if [[ ! "$FORMULA_CLASS" =~ ^[A-Z][0-9A-Za-z_]*$ ]]; then
  echo "FORMULA_CLASS is not a Ruby constant: $FORMULA_CLASS" >&2
  exit 1
fi

# Homebrew derives the class from the file name, so a class naming a different
# formula produces a file it will not load under the name it was written as.
EXPECTED_CLASS="$(printf '%s' "$FORMULA_NAME" | awk -F'[-_]' '{
  out = ""
  for (i = 1; i <= NF; i++) out = out toupper(substr($i, 1, 1)) substr($i, 2)
  print out
}')"
if [[ "$FORMULA_CLASS" != "$EXPECTED_CLASS" ]]; then
  echo "FORMULA_CLASS $FORMULA_CLASS does not match FORMULA_NAME $FORMULA_NAME (expected $EXPECTED_CLASS)" >&2
  exit 1
fi

for field in FORMULA_DESC FORMULA_LICENSE; do
  value="${!field}"
  if [[ -z "$value" || "$value" == *\"* || "$value" == *\\* || "$value" == *"#"* || "$value" == *$'\n'* ]]; then
    echo "$field must be non-empty and free of quotes, backslashes, '#', and newlines: $value" >&2
    exit 1
  fi
done

if [[ -z "$TAP_FORMULA_PATH" && -n "$TAP_REPO_PATH" ]]; then
  # homebrew-core files its formulae under the first character of their name —
  # `Formula/e/exedev-cli.rb` — while a flat personal tap keeps them directly under
  # `Formula`. Writing to the layout the repo does not use produces a file nothing
  # installs from, and the release then reports a tap update that never reached anyone.
  #
  # An existing formula decides it, because that is the file the tap already
  # installs from. The directory is only a hint: a sharded tap has no letter
  # directory until its first formula lands there, and a flat tap can hold an
  # unrelated directory whose name is that letter.
  FORMULA_SHARD_PATH="$TAP_REPO_PATH/Formula/${FORMULA_NAME:0:1}/${FORMULA_NAME}.rb"
  FORMULA_FLAT_PATH="$TAP_REPO_PATH/Formula/${FORMULA_NAME}.rb"
  if [[ -f "$FORMULA_SHARD_PATH" && -f "$FORMULA_FLAT_PATH" ]]; then
    echo "tap has $FORMULA_NAME in both layouts; set TAP_FORMULA_PATH to pick one:" >&2
    echo "  $FORMULA_SHARD_PATH" >&2
    echo "  $FORMULA_FLAT_PATH" >&2
    exit 1
  elif [[ -f "$FORMULA_SHARD_PATH" ]]; then
    TAP_FORMULA_PATH="$FORMULA_SHARD_PATH"
  elif [[ -f "$FORMULA_FLAT_PATH" ]]; then
    TAP_FORMULA_PATH="$FORMULA_FLAT_PATH"
  elif compgen -G "$TAP_REPO_PATH/Formula/${FORMULA_NAME:0:1}/*.rb" > /dev/null; then
    # The directory alone proves nothing; a flat tap can hold an unrelated one.
    # Formulae inside it are what makes the tap sharded.
    TAP_FORMULA_PATH="$FORMULA_SHARD_PATH"
  else
    TAP_FORMULA_PATH="$FORMULA_FLAT_PATH"
  fi
fi

if [[ -z "$TAP_FORMULA_PATH" ]]; then
  echo "TAP_REPO_PATH or TAP_FORMULA_PATH is required" >&2
  exit 1
fi

# The path is created and truncated below, so an explicit value gets the same
# scrutiny as a discovered one: a `../` path or a non-formula target would
# overwrite a file that is not a formula.
if [[ "$TAP_FORMULA_PATH" != *.rb ]]; then
  echo "TAP_FORMULA_PATH must name a .rb formula file: $TAP_FORMULA_PATH" >&2
  exit 1
fi
case "$TAP_FORMULA_PATH" in
  *..*)
    echo "TAP_FORMULA_PATH must not traverse with '..': $TAP_FORMULA_PATH" >&2
    exit 1
    ;;
esac

if [[ -L "$TAP_FORMULA_PATH" ]]; then
  echo "TAP_FORMULA_PATH is a symlink; refusing to write through it: $TAP_FORMULA_PATH" >&2
  exit 1
fi

# An explicit path gets the same confinement as a discovered one when there is a
# tap to confine it to; the file is created and truncated below either way.
if [[ -n "$TAP_REPO_PATH" && -d "$TAP_REPO_PATH" ]]; then
  TAP_REPO_REAL="$(cd "$TAP_REPO_PATH" && pwd -P)"
  FORMULA_PARENT="$(dirname "$TAP_FORMULA_PATH")"
  mkdir -p "$FORMULA_PARENT"
  FORMULA_PARENT_REAL="$(cd "$FORMULA_PARENT" && pwd -P)"
  case "$FORMULA_PARENT_REAL/" in
    "$TAP_REPO_REAL"/*) ;;
    *)
      echo "TAP_FORMULA_PATH resolves outside TAP_REPO_PATH:" >&2
      echo "  formula: $FORMULA_PARENT_REAL" >&2
      echo "  tap:     $TAP_REPO_REAL" >&2
      exit 1
      ;;
  esac
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
# Check the payload against the release we just downloaded instead. Every platform
# is checked: one formula serves all of them, and each `url` is only ever unpacked
# on the platform it belongs to, so a malformed Linux archive is invisible in the
# macOS one.
for platform in "${PLATFORMS[@]}"; do
  # Verbose listing: the formula installs these paths as files, so a directory or
  # a symlink carrying the expected name would satisfy a name-only check and then
  # install the wrong thing.
  tar -tvzf "$WORK_DIR/${ARCHIVE_PREFIX}-${RELEASE_TAG}-${platform}.tar.gz" > "$WORK_DIR/members.txt"
  for member in "${BINARIES[@]}" "${DOCS[@]}"; do
    if ! awk -v want="./$member" '$1 ~ /^-/ && $NF == want { found = 1 } END { exit found ? 0 : 1 }' \
      "$WORK_DIR/members.txt"; then
      echo "$platform release archive has no regular file member: $member" >&2
      exit 1
    fi
  done
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
