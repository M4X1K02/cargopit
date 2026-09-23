#!/usr/bin/env bash
# Print a Debian/RPM-safe version from the GitHub tag that triggered CI.
# A package version must start with a digit; workflow_dispatch on a branch
# and non-version tags fall back so dpkg-deb / rpmbuild still succeed.
set -euo pipefail

readonly SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly VERSION_FILE="${SOURCE_DIR}/version.txt"
readonly FALLBACK_PACKAGE_VERSION="0.0.0"
readonly SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+$'

if [[ ! -f "${VERSION_FILE}" ]]; then
    printf 'Missing release version file: %s\n' "${VERSION_FILE}" >&2
    exit 1
fi

expected_version="$(tr -d '[:space:]' < "${VERSION_FILE}")"
if [[ ! "${expected_version}" =~ ${SEMVER_PATTERN} ]]; then
    printf 'Invalid release version in %s: %s\n' \
        "${VERSION_FILE}" "${expected_version}" >&2
    exit 1
fi

if [ "${GITHUB_REF_TYPE:-}" = tag ]; then
    v="${GITHUB_REF_NAME#v}"
    if [[ ! "${v}" =~ ${SEMVER_PATTERN} ]]; then
        printf 'Release tag must use MAJOR.MINOR.PATCH: %s\n' \
            "${GITHUB_REF_NAME}" >&2
        exit 1
    fi
    if [ "${v}" != "${expected_version}" ]; then
        printf 'Release tag %s does not match version.txt (%s)\n' \
            "${GITHUB_REF_NAME}" "${expected_version}" >&2
        exit 1
    fi
else
    v="${FALLBACK_PACKAGE_VERSION}"
fi

printf '%s\n' "$v"
