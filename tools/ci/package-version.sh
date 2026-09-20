#!/usr/bin/env bash
# Print a Debian/RPM-safe version from the GitHub tag that triggered CI.
# A package version must start with a digit; workflow_dispatch on a branch
# and non-version tags fall back so dpkg-deb / rpmbuild still succeed.
set -euo pipefail

FALLBACK_PACKAGE_VERSION="0.0.0"

v=""
if [ "${GITHUB_REF_TYPE:-}" = tag ]; then
    v="${GITHUB_REF_NAME#v}"
fi
case "$v" in
    [0-9]*) ;;
    *) v="$FALLBACK_PACKAGE_VERSION" ;;
esac
printf '%s\n' "$v"
