#!/bin/bash
# After a source install, prove play mode starts simd without a human launch order.
set -euo pipefail

INSTALL_DIR="${CARGOPIT_INSTALL_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/cargopit}"
BIN_DIR="${HOME}/.local/bin"
PLAY_TIMEOUT_SEC=12
MISSING_TIMEOUT_SEC=6
SIMD_REQUIRED_MSG="simd is required but is not installed"
# Keep in sync with simd_find_binary() in src/cargopit/helper/ensure_simd.c
SIMD_PATH_USR_LOCAL="/usr/local/bin/simd"
SIMD_PATH_USR_BIN="/usr/bin/simd"

export PATH="$BIN_DIR:$PATH"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH:-}:/usr/local/lib:/usr/local/lib64"

CARGOPIT_BIN="${CARGOPIT_BIN:-}"
if [ -z "$CARGOPIT_BIN" ] && [ -x "$INSTALL_DIR/cargopit/build/cargopit" ]; then
    CARGOPIT_BIN="$INSTALL_DIR/cargopit/build/cargopit"
fi
if [ -z "$CARGOPIT_BIN" ] && command -v cargopit >/dev/null 2>&1; then
    CARGOPIT_BIN="$(command -v cargopit)"
fi
if [ -z "$CARGOPIT_BIN" ] || [ ! -x "$CARGOPIT_BIN" ]; then
    echo "cargopit binary not found" >&2
    exit 1
fi

if ! command -v timeout >/dev/null 2>&1; then
    echo "timeout(1) is required" >&2
    exit 1
fi

has_packaged_simd() {
    [ -x "$SIMD_PATH_USR_LOCAL" ] || [ -x "$SIMD_PATH_USR_BIN" ]
}

pkill -x simd 2>/dev/null || true
sleep 1

if has_packaged_simd; then
    echo "SKIP missing simd probe: packaged simd is present after install"
else
    isolated="$(mktemp -d /tmp/cargopit-startup-missing-XXXXXX)"
    mkdir -p "$isolated/.config/cargopit" "$isolated/.cache/cargopit"
    cat > "$isolated/.config/cargopit/cargopit.config" << 'EOF'
configs = (
    {
        sim = "default";
        car = "default";
        devices = ();
    }
);
EOF

    set +e
    missing_out="$(
        env -i \
            HOME="$isolated" \
            XDG_CONFIG_HOME="$isolated/.config" \
            XDG_CACHE_HOME="$isolated/.cache" \
            XDG_DATA_HOME="$isolated/.local/share" \
            PATH="/bin:/usr/bin" \
            TERM=dumb \
            timeout --signal=TERM --kill-after=2 "$MISSING_TIMEOUT_SEC" \
            "$CARGOPIT_BIN" play --disable_audio 2>&1
    )"
    set -e
    rm -rf "$isolated"

    if ! printf '%s\n' "$missing_out" | grep -Fq "$SIMD_REQUIRED_MSG"; then
        echo "FAIL: play mode did not demand simd when it was not installed" >&2
        printf '%s\n' "$missing_out" >&2
        exit 1
    fi
    echo "PASS play mode reports missing simd"
fi

set +e
timeout --signal=TERM --kill-after=2 "$PLAY_TIMEOUT_SEC" \
    "$CARGOPIT_BIN" play --disable_audio >/tmp/cargopit-startup-play.log 2>&1
set -e

if ! pgrep -x simd >/dev/null 2>&1; then
    echo "FAIL: play mode did not start simd" >&2
    tail -n 80 /tmp/cargopit-startup-play.log >&2 || true
    exit 1
fi
echo "PASS play mode started simd"

pkill -x simd 2>/dev/null || true
exit 0
