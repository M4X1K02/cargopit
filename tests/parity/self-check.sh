#!/usr/bin/env bash
# Run the C capture twice per device and require identical output.
set -euo pipefail

CAPTURE="${1:?capture binary}"
ROOT="${2:?source root}"
SCENARIO_BIN="${ROOT}/target/debug/parity-scenarios"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

export PARITY_LUA_FIXTURE="${ROOT}/tests/parity/fixtures/leds.lua"

if [[ ! -x "$SCENARIO_BIN" ]]; then
    cargo build -p parity --bin parity-scenarios --manifest-path "${ROOT}/Cargo.toml"
fi

"$SCENARIO_BIN" dump "${WORKDIR}/scenarios"
mapfile -t DEVICES < <("$CAPTURE" --list)

for device in "${DEVICES[@]}"; do
    "$CAPTURE" --device "$device" --scenario "${WORKDIR}/scenarios/basic.simdata" \
        --output "${WORKDIR}/a.golden"
    "$CAPTURE" --device "$device" --scenario "${WORKDIR}/scenarios/basic.simdata" \
        --output "${WORKDIR}/b.golden"
    if ! cmp -s "${WORKDIR}/a.golden" "${WORKDIR}/b.golden"; then
        echo "capture was not deterministic for ${device}" >&2
        diff -u "${WORKDIR}/a.golden" "${WORKDIR}/b.golden" >&2 || true
        exit 1
    fi
    if [[ ! -s "${WORKDIR}/a.golden" ]]; then
        echo "capture produced an empty log for ${device}" >&2
        exit 1
    fi
done

echo "parity self-check passed (${#DEVICES[@]} devices)"
