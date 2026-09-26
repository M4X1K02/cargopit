#!/usr/bin/env bash
# Level 2 virtual jobs for the shadow release. Hardware acceptance stays with a maintainer.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RUST_BIN="${CARGOPIT_BIN:-$ROOT/target/debug/cargopit}"
if [ ! -x "$RUST_BIN" ]; then
    cargo build -p cargopit --bin cargopit
fi

# One disabled USB tachometer: the host refuses a missing config, and an empty
# device list logs an error instead of a test step.
SHADOW_TEST_CONFIG='configs = (
    {
        sim = "default";
        car = "default";
        devices = (
            {
                device = "USB";
                type = "Tachometer";
                enabled = false;
            }
        );
    }
);
'
TEST_STEP_PREFIX="test step: "

bash tests/version_reporting.sh "$RUST_BIN" "$(tr -d '[:space:]' < version.txt)"
bash tests/startup_play.sh missing "$RUST_BIN"
bash tests/startup_play.sh starts "$RUST_BIN"

shadow_home="$(mktemp -d /tmp/cargopit-shadow-XXXXXX)"
cleanup() {
    if [ -n "${shadow_home:-}" ]; then
        rm -rf "$shadow_home"
    fi
}
trap cleanup EXIT
mkdir -p "$shadow_home/.config/cargopit" "$shadow_home/.cache"
config_path="$shadow_home/.config/cargopit/cargopit.config"
printf '%s\n' "$SHADOW_TEST_CONFIG" > "$config_path"

out="$(
    HOME="$shadow_home" \
    XDG_CACHE_HOME="$shadow_home/.cache" \
    "$RUST_BIN" test --disable_audio -c "$config_path"
)"
if ! printf '%s\n' "$out" | grep -Fq "$TEST_STEP_PREFIX"; then
    echo "FAIL test: config did not produce a test step" >&2
    printf '%s\n' "$out" >&2
    exit 1
fi

if [ -x "$ROOT/build/cargopit-legacy" ]; then
    bash tests/version_reporting.sh "$ROOT/build/cargopit-legacy" "$(tr -d '[:space:]' < version.txt)"
fi

echo "PASS shadow virtual jobs"
