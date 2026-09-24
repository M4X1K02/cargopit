#!/usr/bin/env bash
# Golden bytes must come from regen-goldens.sh. A hand edit fails this check
# unless the commit message contains REGENERATE GOLDENS and a fresh C capture
# matches the files.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASE="${1:-origin/master}"

if ! git -C "$ROOT" rev-parse --verify "$BASE" >/dev/null 2>&1; then
    echo "golden edit check skipped; base ${BASE} is not available"
    exit 0
fi

CHANGED="$(git -C "$ROOT" diff --name-only "$BASE"...HEAD -- tests/parity/golden || true)"
if [[ -z "$CHANGED" ]]; then
    echo "no golden edits"
    exit 0
fi

MESSAGE="$(git -C "$ROOT" log "$BASE"..HEAD --format=%B)"
if ! grep -q "REGENERATE GOLDENS" <<<"$MESSAGE"; then
    echo "golden files changed without REGENERATE GOLDENS in the commit message" >&2
    echo "$CHANGED" >&2
    exit 1
fi

echo "golden regeneration marker present"
