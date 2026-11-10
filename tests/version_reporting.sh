#!/usr/bin/env bash
set -euo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly VERSION_FILE="${REPO_ROOT}/version.txt"
readonly CARGO_TOML="${REPO_ROOT}/Cargo.toml"
readonly SEMVER_PATTERN='^[0-9]+\.[0-9]+\.[0-9]+$'
readonly CARGOPIT_BINARY_NAME="cargopit"
readonly TUI_BINARY_NAME="cargopit-tui"

readonly BINARY="${1:?cargopit binary path required}"
readonly EXPECTED_VERSION="${2:?expected version required}"
readonly TUI_BINARY_ARG="${3:-}"

die() {
    printf '%s\n' "$*" >&2
    exit 1
}

read_trimmed_file() {
    local path="$1"
    if [[ ! -f "${path}" ]]; then
        die "Missing file: ${path}"
    fi
    tr -d '[:space:]' < "${path}"
}

assert_semver() {
    local label="$1"
    local value="$2"
    if [[ ! "${value}" =~ ${SEMVER_PATTERN} ]]; then
        die "Invalid ${label} version '${value}' (want MAJOR.MINOR.PATCH)"
    fi
}

workspace_cargo_version() {
    awk '
        /^\[workspace\.package\]/ { in_ws = 1; next }
        /^\[/ { if (in_ws) exit }
        in_ws && /^version = / {
            gsub(/^version = "|"$/, "", $0)
            print $0
            exit
        }
    ' "${CARGO_TOML}"
}

assert_version_output() {
    local binary="$1"
    local program_name="$2"
    local version="$3"
    if [[ ! -x "${binary}" ]]; then
        die "Binary is not executable: ${binary}"
    fi
    local expected_output="${program_name} ${version}"
    local output="$("${binary}" --version 2>&1)"
    if [[ "${output}" != *"${expected_output}"* ]]; then
        printf 'Expected %s output containing "%s", got:\n%s\n' \
            "${program_name}" "${expected_output}" "${output}" >&2
        exit 1
    fi
}

version_from_sources="$(read_trimmed_file "${VERSION_FILE}")"
assert_semver "version.txt" "${version_from_sources}"

cargo_version="$(workspace_cargo_version)"
if [[ -z "${cargo_version}" ]]; then
    die "Could not read workspace.package version from ${CARGO_TOML}"
fi
assert_semver "Cargo.toml workspace" "${cargo_version}"

if [[ "${version_from_sources}" != "${cargo_version}" ]]; then
    die "version.txt (${version_from_sources}) does not match Cargo.toml (${cargo_version})"
fi

if [[ "${EXPECTED_VERSION}" != "${version_from_sources}" ]]; then
    die "Expected version ${EXPECTED_VERSION} does not match version.txt (${version_from_sources})"
fi

assert_version_output "${BINARY}" "${CARGOPIT_BINARY_NAME}" "${version_from_sources}"

tui_binary="${TUI_BINARY_ARG}"
if [[ -z "${tui_binary}" ]]; then
    candidate="$(dirname "${BINARY}")/${TUI_BINARY_NAME}"
    if [[ -x "${candidate}" ]]; then
        tui_binary="${candidate}"
    fi
fi

if [[ -n "${tui_binary}" && -x "${tui_binary}" ]]; then
    assert_version_output "${tui_binary}" "${TUI_BINARY_NAME}" "${version_from_sources}"
fi
