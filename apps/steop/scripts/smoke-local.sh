#!/usr/bin/env bash
# Go CLI smoke tests for the local storage surface: state, storage, session,
# status, monitor, statusline. Mailbox/notify smoke stays in
# docs/steop/smoke-tests.md — those RPCs still go to stele-server.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STEOP_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

export STEOP_DB="$TMPDIR/steop.db"
export CLAUDE_PROJECT_DIR="/tmp/smoke-local"
export STELE_HOST="smoke-host"

BIN="$TMPDIR/steop"
(cd "$STEOP_DIR" && CGO_ENABLED=0 go build -o "$BIN" .)

SESSION="a1b2c3d4-5678-4abc-9def-0123456789ab"

echo "--- db init"
"$BIN" db init

echo "--- state set / get"
"$BIN" state set "$SESSION" '{"phase":"plan","mode":"flow"}'
"$BIN" state get "$SESSION"

echo "--- state incr"
"$BIN" state incr "$SESSION" tool_calls
"$BIN" state incr "$SESSION" tool_calls 4

echo "--- state reset"
"$BIN" state reset "$SESSION" loop_count 2

echo "--- storage put / get / list (project scope)"
"$BIN" storage put hello world
"$BIN" storage get hello
"$BIN" storage list

echo "--- storage put / get / list (session scope)"
"$BIN" storage --session="$SESSION" put task '{"id":"t1"}'
"$BIN" storage --session="$SESSION" list

echo "--- monitor (list all)"
"$BIN" monitor --json

echo "--- monitor inspect"
"$BIN" monitor --json "$SESSION"

echo "--- statusline (json)"
"$BIN" statusline --json --session="$SESSION" </dev/null

echo "--- state delete"
"$BIN" state delete "$SESSION"

echo "PASS: local smoke green"
