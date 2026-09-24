#!/usr/bin/env bash
# Regenerate tests/parity/golden from the unmodified C capture.
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
    echo "missing ${CAPTURE}; configure with -DENABLE_TESTS=ON and build first" >&2
    exit 1
fi

cargo build -p parity --bin parity-scenarios --manifest-path "${ROOT}/Cargo.toml"
"$SCENARIO_BIN" dump "${WORKDIR}/scenarios"
mkdir -p "$GOLDEN"
find "$GOLDEN" -name '*.golden' -delete

mapfile -t DEVICES < <("$CAPTURE" --list)
for device in "${DEVICES[@]}"; do
    for scenario in "${WORKDIR}/scenarios/"*.simdata; do
        name="$(basename "$scenario" .simdata)"
        "$CAPTURE" --device "$device" --scenario "$scenario" \
            --output "${GOLDEN}/${device}__${name}.golden"
    done
done

echo "regenerated $(find "$GOLDEN" -name '*.golden' | wc -l) golden files"
