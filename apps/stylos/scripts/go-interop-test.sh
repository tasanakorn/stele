#!/usr/bin/env bash
# Go<->Rust interop smoke test for PRD-019 Pass B-Go.
#
# Scenario:
#   1. Rust queryable on stylos/dev/poc/echo (listens tcp/127.0.0.1:47447)
#   2. Go get stylos/dev/poc/echo --connect tcp/127.0.0.1:47447
#   3. Assert Go stdout contains "hello-from-rust"

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_BIN="$HERE/target/debug/stylos"
GO_DIR="$HERE/go"
GO_BIN="$GO_DIR/target/stylos"
TMP="$(mktemp -d -t stylos-go-interop.XXXXXX)"
QUERYABLE_PORT=47447
KEY_ECHO="stylos/dev/poc/echo"
PAYLOAD_ECHO="hello-from-rust"

PREFIX="$HERE/third_party/zenoh-c-prefix"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export CGO_CFLAGS="-I$PREFIX/include"
export CGO_LDFLAGS="-L$PREFIX/lib -lzenohc -Wl,-rpath,$PREFIX/lib"

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

log() { printf "\033[36m[interop]\033[0m %s\n" "$*"; }
ok()  { printf "\033[32m  PASS\033[0m %s\n" "$*"; }
bad() { printf "\033[31m  FAIL\033[0m %s\n" "$*"; fail=1; }

# ---- Build Go binary ----
log "building Go binary"
if (cd "$GO_DIR" && ./build.sh) 2>&1; then
  ok "go build succeeded"
else
  bad "go build failed"
  exit 1
fi

# ---- Check Rust binary ----
log "checking Rust binary: $RUST_BIN"
[[ -x "$RUST_BIN" ]] || { bad "Rust binary not found; run 'cargo build' in apps/stylos/"; exit 1; }

# ---- Write Rust queryable config ----
CFG_Q="$TMP/queryable.json5"
cat > "$CFG_Q" <<EOF
{
  stylos: {
    realm:    "dev",
    role:     "smoke",
    instance: "queryable-go-interop",
  },
  zenoh: {
    mode: "peer",
    listen: {
      endpoints: ["tcp/0.0.0.0:${QUERYABLE_PORT}"],
    },
    scouting: {
      multicast: { enabled: false, address: "224.0.0.224:31746", interface: "auto" },
      gossip:    { enabled: true },
    },
  },
}
EOF

# ---- Start Rust queryable ----
Q_OUT="$TMP/queryable.log"
log "starting Rust queryable on $KEY_ECHO, port $QUERYABLE_PORT"
"$RUST_BIN" queryable "$KEY_ECHO" --payload "$PAYLOAD_ECHO" --config "$CFG_Q" > "$Q_OUT" 2>&1 &
Q_PID=$!
BG_PIDS+=("$Q_PID")
sleep 1
kill -0 "$Q_PID" 2>/dev/null && ok "queryable pid=$Q_PID" || { bad "queryable exited early"; cat "$Q_OUT"; exit 1; }

# ---- Run Go get ----
log "running Go get on $KEY_ECHO"
G_OUT="$TMP/go-get.log"
if "$GO_BIN" get --connect "tcp/127.0.0.1:${QUERYABLE_PORT}" --timeout-ms 5000 "$KEY_ECHO" > "$G_OUT" 2>&1; then
  if grep -q "$PAYLOAD_ECHO" "$G_OUT"; then
    ok "Go received '$PAYLOAD_ECHO'"
  else
    bad "Go get succeeded but output did not contain '$PAYLOAD_ECHO'"
    echo "--- go get output ---"
    cat "$G_OUT"
  fi
else
  bad "Go get non-zero exit"
  echo "--- go get output ---"
  cat "$G_OUT"
fi

# ---- Shutdown ----
log "shutting down Rust queryable"
kill "$Q_PID" 2>/dev/null || true
sleep 0.5
kill -0 "$Q_PID" 2>/dev/null && bad "queryable did not exit on SIGTERM" || ok "queryable exited"

echo
if [[ $fail -eq 0 ]]; then
  printf "\033[32mGO INTEROP TEST PASSED\033[0m\n"
  exit 0
else
  printf "\033[31mGO INTEROP TEST FAILED\033[0m\n"
  exit 1
fi
