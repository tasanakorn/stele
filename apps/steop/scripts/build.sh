#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STEOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$STEOP_DIR/../.." && pwd)"
OUT_DIR="$REPO_ROOT/plugins/steop/bin"

mkdir -p "$OUT_DIR"

cd "$STEOP_DIR"
VERSION="$(grep -E '^const Version' version.go | sed -E 's/.*"([^"]+)".*/\1/')"

CGO_ENABLED=0 go build \
    -trimpath \
    -ldflags="-s -w -X main.Version=${VERSION}" \
    -o "$OUT_DIR/steop" \
    .

echo "Built: $OUT_DIR/steop"
"$OUT_DIR/steop" version
