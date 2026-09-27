#!/usr/bin/env bash
# Fail unless committed goldens match a fresh Rust capture.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

export PARITY_LUA_FIXTURE="${ROOT}/tests/parity/fixtures/leds.lua"

cargo test --manifest-path "${ROOT}/Cargo.toml" -p parity --lib
