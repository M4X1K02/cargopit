#!/usr/bin/env bash
# Direct clock reads belong in crates/cargopit-devices/src/clock.rs.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PATTERN='std::time::(Instant|SystemTime)|clock_gettime|gettimeofday|libc::clock'
HITS="$(grep -R -n -E "$PATTERN" "${ROOT}/crates" \
    --include='*.rs' --exclude='clock.rs' || true)"

if [[ -n "$HITS" ]]; then
    echo "clock reads outside the clock module:" >&2
    echo "$HITS" >&2
    exit 1
fi

echo "clock check passed"
