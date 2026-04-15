#!/usr/bin/env bash
# Go<->Rust interop smoke test for PRD-019 Pass B-Go (reverse direction).
#
# Scenario:
#   1. Rust subscriber on stylos/dev/poc/go (listens tcp/127.0.0.1:47448)
#   2. Go pub stylos/dev/poc/go "hello-from-go" --connect tcp/127.0.0.1:47448
#   3. Assert Rust subscriber log contains "hello-from-go" within 5s
#
# Validates PRD-019 §4.8.4 criterion 2.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_BIN="$HERE/target/debug/stylos"
GO_DIR="$HERE/go"
GO_BIN="$GO_DIR/target/stylos"
TMP="$(mktemp -d -t stylos-go-pub-rust-sub.XXXXXX)"
SUB_PORT=47448
KEY="stylos/dev/poc/go"
PAYLOAD="hello-from-go"

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

# ---- Write Rust subscriber config ----
CFG_S="$TMP/sub.json5"
cat > "$CFG_S" <<EOF
{
  stylos: {
    realm:    "dev",
    role:     "smoke",
    instance: "sub-go-interop",
  },
  zenoh: {
    mode: "peer",
    listen: {
      endpoints: ["tcp/0.0.0.0:${SUB_PORT}"],
    },
    scouting: {
      multicast: { enabled: false, address: "224.0.0.224:31746", interface: "auto" },
      gossip:    { enabled: true },
    },
  },
}
EOF

# ---- Start Rust subscriber ----
S_OUT="$TMP/sub.log"
log "starting Rust subscriber on $KEY, port $SUB_PORT"
"$RUST_BIN" sub "$KEY" --config "$CFG_S" > "$S_OUT" 2>&1 &
S_PID=$!
BG_PIDS+=("$S_PID")
sleep 1
kill -0 "$S_PID" 2>/dev/null && ok "subscriber pid=$S_PID" || { bad "subscriber exited early"; cat "$S_OUT"; exit 1; }

# ---- Run Go pub ----
log "running Go pub on $KEY with payload '$PAYLOAD'"
P_OUT="$TMP/pub.log"
if "$GO_BIN" pub --connect "tcp/127.0.0.1:${SUB_PORT}" "$KEY" "$PAYLOAD" > "$P_OUT" 2>&1; then
  ok "Go pub returned 0"
else
  bad "Go pub non-zero exit"
  echo "--- go pub output ---"
  cat "$P_OUT"
fi

# ---- Poll subscriber log up to 5s ----
log "waiting up to 5s for Rust subscriber to print '$PAYLOAD'"
received=0
for _ in $(seq 1 10); do
  if grep -q "$PAYLOAD" "$S_OUT"; then received=1; break; fi
  sleep 0.5
done
if [[ $received -eq 1 ]]; then
  ok "Rust subscriber received '$PAYLOAD'"
else
  bad "Rust subscriber did not receive '$PAYLOAD' within 5s"
  echo "--- subscriber log ---"
  cat "$S_OUT"
fi

# ---- Shutdown ----
log "shutting down Rust subscriber"
kill "$S_PID" 2>/dev/null || true
sleep 0.5
kill -0 "$S_PID" 2>/dev/null && bad "subscriber did not exit on SIGTERM" || ok "subscriber exited"

echo
if [[ $fail -eq 0 ]]; then
  printf "\033[32mGO-PUB -> RUST-SUB INTEROP PASSED\033[0m\n"
  exit 0
else
  printf "\033[31mGO-PUB -> RUST-SUB INTEROP FAILED\033[0m\n"
  exit 1
fi
