#!/usr/bin/env bash
# Stylos POC smoke test (Pass B-Rust).
#
# Runs the PRD-019 §4.8 acceptance scenario on a single host:
#   1. queryable on stylos/dev/poc/echo
#   2. subscriber on stylos/dev/poc/rust
#   3. publisher → stylos/dev/poc/rust
#   4. get stylos/dev/poc/echo
#
# Uses explicit tcp/127.0.0.1 connects to sidestep macOS multicast loopback flakiness.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$HERE/target/debug/stylos"
TMP="$(mktemp -d -t stylos-smoke.XXXXXX)"
QUERYABLE_PORT=31747
SUB_PORT=31748
PUB_PORT=31749
GET_PORT=31750
CONNECT="tcp/127.0.0.1:${QUERYABLE_PORT}"
KEY_PUB="stylos/dev/poc/rust"
KEY_ECHO="stylos/dev/poc/echo"
PAYLOAD_PUB="hello-from-rust"
PAYLOAD_ECHO="reply-from-rust"

fail=0
declare -a BG_PIDS=()

cleanup() {
  for pid in "${BG_PIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

log()  { printf "\033[36m[smoke]\033[0m %s\n" "$*"; }
ok()   { printf "\033[32m  PASS\033[0m %s\n" "$*"; }
bad()  { printf "\033[31m  FAIL\033[0m %s\n" "$*"; fail=1; }

# Pin realm/role/instance so identity validation is deterministic.
write_cfg() {
  local path="$1" instance="$2" port="$3"
  cat > "$path" <<EOF
{
  stylos: {
    realm:    "dev",
    role:     "smoke",
    instance: "${instance}",
  },
  zenoh: {
    mode: "peer",
    listen: {
      endpoints: ["tcp/0.0.0.0:${port}"],
    },
    scouting: {
      multicast: { enabled: false, address: "224.0.0.224:31746", interface: "auto" },
      gossip:    { enabled: true },
    },
  },
}
EOF
}

# ---- Precondition ----
log "checking binary: $BIN"
[[ -x "$BIN" ]] || { bad "binary not found; run 'cargo build' in apps/stylos/"; exit 1; }
"$BIN" --version && ok "binary runs"

# ---- 1. Queryable ----
CFG_Q="$TMP/queryable.json5"; write_cfg "$CFG_Q" "queryable-01" "$QUERYABLE_PORT"
Q_OUT="$TMP/queryable.log"
log "starting queryable on $KEY_ECHO, listening tcp/0.0.0.0:${QUERYABLE_PORT}"
"$BIN" queryable "$KEY_ECHO" --payload "$PAYLOAD_ECHO" --config "$CFG_Q" > "$Q_OUT" 2>&1 &
Q_PID=$!; BG_PIDS+=("$Q_PID")
sleep 1
kill -0 "$Q_PID" 2>/dev/null && ok "queryable pid=$Q_PID" || { bad "queryable exited early"; cat "$Q_OUT"; exit 1; }

# ---- 2. Subscriber ----
CFG_S="$TMP/sub.json5"; write_cfg "$CFG_S" "sub-01" "$SUB_PORT"
S_OUT="$TMP/sub.log"
log "starting subscriber on $KEY_PUB, connecting to $CONNECT"
"$BIN" sub "$KEY_PUB" --config "$CFG_S" --connect "$CONNECT" > "$S_OUT" 2>&1 &
S_PID=$!; BG_PIDS+=("$S_PID")
sleep 2
kill -0 "$S_PID" 2>/dev/null && ok "subscriber pid=$S_PID" || { bad "subscriber exited early"; cat "$S_OUT"; exit 1; }

# ---- 3. Publish ----
CFG_P="$TMP/pub.json5"; write_cfg "$CFG_P" "pub-01" "$PUB_PORT"
log "publishing $PAYLOAD_PUB -> $KEY_PUB"
if "$BIN" pub "$KEY_PUB" "$PAYLOAD_PUB" --config "$CFG_P" --connect "$CONNECT" > "$TMP/pub.log" 2>&1; then
  ok "pub returned 0"
else
  bad "pub non-zero exit"; cat "$TMP/pub.log"
fi

# ---- 4. Verify subscriber received the sample ----
log "waiting up to 5s for subscriber to print sample"
received=0
for _ in $(seq 1 10); do
  if grep -q "$PAYLOAD_PUB" "$S_OUT"; then received=1; break; fi
  sleep 0.5
done
if [[ $received -eq 1 ]]; then
  ok "subscriber received '$PAYLOAD_PUB'"
else
  bad "subscriber did not receive '$PAYLOAD_PUB' within 5s"
  echo "--- subscriber log ---"; cat "$S_OUT"
fi

# ---- 5. Get from queryable ----
CFG_G="$TMP/get.json5"; write_cfg "$CFG_G" "get-01" "$GET_PORT"
log "querying $KEY_ECHO"
G_OUT="$TMP/get.log"
if "$BIN" get "$KEY_ECHO" --timeout-ms 3000 --config "$CFG_G" --connect "$CONNECT" > "$G_OUT" 2>&1; then
  if grep -q "$PAYLOAD_ECHO" "$G_OUT"; then
    ok "get received '$PAYLOAD_ECHO'"
  else
    bad "get did not receive '$PAYLOAD_ECHO'"
    echo "--- get log ---"; cat "$G_OUT"
  fi
else
  bad "get non-zero exit"; cat "$G_OUT"
fi

# ---- 6. Shutdown hygiene ----
log "shutting down background processes"
kill "$Q_PID" "$S_PID" 2>/dev/null || true
sleep 0.5
kill -0 "$Q_PID" 2>/dev/null && bad "queryable did not exit on SIGTERM" || ok "queryable exited cleanly"
kill -0 "$S_PID" 2>/dev/null && bad "subscriber did not exit on SIGTERM" || ok "subscriber exited cleanly"

echo
if [[ $fail -eq 0 ]]; then
  printf "\033[32mSMOKE TEST PASSED\033[0m\n"
  exit 0
else
  printf "\033[31mSMOKE TEST FAILED\033[0m (logs under $TMP retained? no, cleaned up by trap)\n"
  exit 1
fi
