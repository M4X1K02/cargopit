#!/usr/bin/env bash
# Fail unless committed goldens match a fresh run of the C capture.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUILD="${1:-${ROOT}/build}"
CAPTURE="${BUILD}/tests/parity/parity_c_capture"
GOLDEN="${ROOT}/tests/parity/golden"
SCENARIO_BIN="${ROOT}/target/debug/parity-scenarios"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

export PARITY_LUA_FIXTURE="${ROOT}/tests/parity/fixtures/leds.lua"

if [[ ! -x "$CAPTURE" ]]; then
    echo "missing ${CAPTURE}" >&2
    exit 1
fi
if ! compgen -G "${GOLDEN}/*.golden" >/dev/null; then
    echo "no golden files; run tests/parity/regen-goldens.sh" >&2
    exit 1
fi

if [[ ! -x "$SCENARIO_BIN" ]]; then
    cargo build -p parity --bin parity-scenarios --manifest-path "${ROOT}/Cargo.toml"
fi
"$SCENARIO_BIN" dump "${WORKDIR}/scenarios"

for golden in "${GOLDEN}/"*.golden; do
    base="$(basename "$golden" .golden)"
    device="${base%%__*}"
    scenario="${base#*__}"
    "$CAPTURE" --device "$device" --scenario "${WORKDIR}/scenarios/${scenario}.simdata" \
        --output "${WORKDIR}/fresh.golden"
    if ! cmp -s "$golden" "${WORKDIR}/fresh.golden"; then
        echo "golden mismatch: ${base}" >&2
        diff -u "$golden" "${WORKDIR}/fresh.golden" | head -80 >&2 || true
        exit 1
    fi
done

echo "goldens match the C capture"
