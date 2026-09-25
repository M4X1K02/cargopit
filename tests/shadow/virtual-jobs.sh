#!/usr/bin/env bash
# Level 2 virtual jobs for the shadow release. Hardware acceptance stays with a maintainer.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RUST_BIN="${CARGOPIT_BIN:-$ROOT/target/debug/cargopit}"
if [ ! -x "$RUST_BIN" ]; then
    cargo build -p cargopit --bin cargopit
fi

bash tests/version_reporting.sh "$RUST_BIN" "$(tr -d '[:space:]' < version.txt)"
bash tests/startup_play.sh missing "$RUST_BIN"
bash tests/startup_play.sh starts "$RUST_BIN"

out="$("$RUST_BIN" test --disable_audio)"
printf '%s\n' "$out" | grep -Fq "test step: "

if [ -x "$ROOT/build/cargopit-legacy" ]; then
    bash tests/version_reporting.sh "$ROOT/build/cargopit-legacy" "$(tr -d '[:space:]' < version.txt)"
fi

echo "PASS shadow virtual jobs"
