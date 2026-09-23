#!/usr/bin/env bash
set -euo pipefail

readonly BINARY="$1"
readonly EXPECTED_VERSION="$2"
readonly EXPECTED_OUTPUT="cargopit ${EXPECTED_VERSION}"

output="$("${BINARY}" --version 2>&1)"
if [[ "${output}" != *"${EXPECTED_OUTPUT}"* ]]; then
    printf 'Expected version output containing "%s", got:\n%s\n' \
        "${EXPECTED_OUTPUT}" "${output}" >&2
    exit 1
fi
