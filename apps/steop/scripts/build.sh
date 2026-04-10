#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STEOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="${OUT_DIR:-$HOME/.local/bin}"

mkdir -p "$OUT_DIR"

cd "$STEOP_DIR"

# Version comes from the `const Version` in version.go. We do not pass it via
# `-ldflags -X` because `-X` only overrides string vars, not consts, so it
# would be a no-op.
CGO_ENABLED=0 go build \
    -trimpath \
    -ldflags="-s -w" \
    -o "$OUT_DIR/steop" \
    .

echo "Built: $OUT_DIR/steop"
"$OUT_DIR/steop" version
