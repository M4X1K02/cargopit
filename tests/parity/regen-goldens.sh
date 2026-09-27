#!/usr/bin/env bash
# Regenerate tests/parity/golden from the Rust device captures.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GOLDEN="${1:-${ROOT}/tests/parity/golden}"

export PARITY_LUA_FIXTURE="${ROOT}/tests/parity/fixtures/leds.lua"

cargo run --manifest-path "${ROOT}/Cargo.toml" -p parity --bin parity-scenarios -- regen "$GOLDEN"
