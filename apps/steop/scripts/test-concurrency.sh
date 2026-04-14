#!/usr/bin/env bash
# Fires N parallel `steop state incr tool_calls` against a fresh DB and
# asserts the final counter equals N. Exercises PRD-020 §8.5: the
# BEGIN IMMEDIATE + 5s busy_timeout path under contention across processes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STEOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
N="${N:-16}"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

export STEOP_DB="$TMPDIR/steop.db"
export CLAUDE_PROJECT_DIR="/tmp/test-concurrency"
export STELE_HOST="test-host"

# Build the binary into a tmpdir to avoid clobbering the user's install.
BIN="$TMPDIR/steop"
(cd "$STEOP_DIR" && CGO_ENABLED=0 go build -o "$BIN" .)

SESSION="a1b2c3d4-5678-4abc-9def-0123456789ab"

# Pre-create the DB so the first increment doesn't race against schema init.
"$BIN" db init >/dev/null

seq 1 "$N" | xargs -P "$N" -I{} "$BIN" state incr "$SESSION" tool_calls >/dev/null

GOT=$("$BIN" state get "$SESSION" | python3 -c 'import sys, json; d = json.load(sys.stdin); print(d["counters"]["tool_calls"])')

if [ "$GOT" != "$N" ]; then
    echo "FAIL: tool_calls = $GOT, want $N"
    exit 1
fi

echo "PASS: $N parallel increments landed as $GOT"
