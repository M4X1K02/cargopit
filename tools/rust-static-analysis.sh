#!/usr/bin/env bash
# Clippy and rustfmt for the Rust workspace, including cargopit-tui.
set -euo pipefail

readonly SCRIPT_NAME="$(basename "$0")"
readonly SOURCE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly CLIPPY_DENY_FLAGS="-D warnings"

usage() {
    cat <<EOF
Usage: ${SCRIPT_NAME} [options]

Run Clippy with warnings denied and rustfmt --check on the Cargo workspace.

Options:
  -h, --help  Show this help
EOF
}

fail() {
    printf '%s: %s\n' "${SCRIPT_NAME}" "$1" >&2
    exit 2
}

while (($# > 0)); do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1"
            ;;
    esac
done

cargo_command="${CARGO:-cargo}"
command -v "${cargo_command}" >/dev/null 2>&1 ||
    fail "cargo is required"
"${cargo_command}" clippy --version >/dev/null 2>&1 ||
    fail "cargo clippy is required (rustup component add clippy)"
"${cargo_command}" fmt --version >/dev/null 2>&1 ||
    fail "cargo fmt is required (rustup component add rustfmt)"

cd "${SOURCE_DIR}"
"${cargo_command}" clippy --workspace --all-targets -- ${CLIPPY_DENY_FLAGS}
"${cargo_command}" fmt --all -- --check
printf 'Rust static analysis passed.\n'
