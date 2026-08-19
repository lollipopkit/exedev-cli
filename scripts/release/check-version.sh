#!/usr/bin/env bash
set -euo pipefail

# Validates a release version or tag against semver.org's reference grammar and
# prints the version with any leading `v` removed.
#
# Kept apart from set-version.sh so the release workflow can reject a bad tag at
# the point it is resolved, before four matrix builds check out and install a
# toolchain only to fail on the same string.

# `${1:-...}` would treat an explicitly passed empty tag as no argument at all and
# validate RELEASE_TAG instead, reporting success for a version the caller never
# asked about.
if [[ $# -ge 1 ]]; then
  VERSION="$1"
else
  VERSION="${RELEASE_TAG:-}"
fi

if [[ -z "$VERSION" ]]; then
  echo "usage: $(basename "$0") <version>" >&2
  echo "Accepts either 0.1.22 or v0.1.22; RELEASE_TAG is used when no argument is given." >&2
  exit 1
fi

VERSION="${VERSION#v}"

# Character ranges in the pattern below are ASCII; a locale with different
# collation would otherwise decide what [0-9A-Za-z] covers.
LC_ALL=C

SEMVER_NUM='(0|[1-9][0-9]*)'
SEMVER_PRE_ID="(${SEMVER_NUM}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER_RE="^${SEMVER_NUM}\.${SEMVER_NUM}\.${SEMVER_NUM}(-${SEMVER_PRE_ID}(\.${SEMVER_PRE_ID})*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$"

if [[ ! "$VERSION" =~ $SEMVER_RE ]]; then
  echo "not a semantic version: $VERSION" >&2
  echo "Expected something like v0.1.11 or 1.2.3-rc.1+build.5." >&2
  exit 1
fi

printf '%s\n' "$VERSION"
