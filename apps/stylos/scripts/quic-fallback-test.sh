#!/usr/bin/env bash
# Stylos QUIC-fallback test (PRD-019 §4.8.4 criterion 4).
#
# Peer A: QUIC+TCP listener on 47447, runs queryable on stylos/dev/poc/echo.
# Peer B: TCP-only (--no-quic), connects explicitly to Peer A on tcp/127.0.0.1:47447.
# Verifies: Peer A binds a QUIC listener; Peer B receives a reply over TCP.
#
# NOTE: Fixed ports — re-run may fail if 47447/47448 are still held from a prior run.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$HERE/target/debug/stylos"
CERTS="$HERE/certs"
TMP="$(mktemp -d -t stylos-quic-fallback.XXXXXX)"
PEER_A_PORT=47447
PEER_B_PORT=47448
KEY_ECHO="stylos/dev/poc/echo"
PAYLOAD_ECHO="reply-from-rust"

fail=0
declare -a BG_PIDS=()

cleanup() {
  for pid in "${BG_PIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  if [[ "${STYLOS_KEEP_LOGS:-0}" == "1" ]]; then
    printf "\033[36m[quic-fallback]\033[0m logs retained at %s\n" "$TMP"
  else
    rm -rf "$TMP"
  fi
}
trap cleanup EXIT

log()  { printf "\033[36m[quic-fallback]\033[0m %s\n" "$*"; }
ok()   { printf "\033[32m  PASS\033[0m %s\n" "$*"; }
bad()  { printf "\033[31m  FAIL\033[0m %s\n" "$*"; fail=1; }
warn() { printf "\033[33m  WARN\033[0m %s\n" "$*"; }

# ---- Precondition ----
log "checking binary: $BIN"
[[ -x "$BIN" ]] || { bad "binary not found; run 'cargo build' in apps/stylos/"; exit 1; }
"$BIN" --version && ok "binary runs"

# ---- Cert setup (idempotent) ----
if [[ -f "$CERTS/stylos-dev.key" && -f "$CERTS/stylos-dev.crt" && -f "$CERTS/stylos-dev-ca.crt" ]]; then
  log "certs already exist at $CERTS — skipping gen-dev-certs.sh"
else
  log "certs not found — running gen-dev-certs.sh"
  bash "$HERE/scripts/gen-dev-certs.sh"
fi
ok "certs ready"

# ---- Write configs ----
CFG_A="$TMP/peer-a.json5"
CFG_B="$TMP/peer-b.json5"

cat > "$CFG_A" <<EOF
{
  stylos: {
    realm:    "dev",
    role:     "quic-fallback",
    instance: "peer-a",
  },
  zenoh: {
    mode: "peer",
    listen: {
      endpoints: ["quic/0.0.0.0:${PEER_A_PORT}", "tcp/0.0.0.0:${PEER_A_PORT}"],
    },
    scouting: {
      multicast: { enabled: false },
      gossip:    { enabled: false },
    },
    transport: {
      link: {
        tls: {
          listen_private_key:  "${CERTS}/stylos-dev.key",
          listen_certificate:  "${CERTS}/stylos-dev.crt",
          root_ca_certificate: "${CERTS}/stylos-dev-ca.crt",
        },
      },
    },
  },
}
EOF

cat > "$CFG_B" <<EOF
{
  stylos: {
    realm:    "dev",
    role:     "quic-fallback",
    instance: "peer-b",
  },
  zenoh: {
    mode: "peer",
    listen: {
      endpoints: ["tcp/0.0.0.0:${PEER_B_PORT}"],
    },
    scouting: {
      multicast: { enabled: false },
      gossip:    { enabled: false },
    },
  },
}
EOF

log "configs written to $TMP"

# ---- Start Peer A (QUIC+TCP queryable) ----
A_OUT="$TMP/peer-a.log"
log "starting Peer A (QUIC+TCP) queryable on $KEY_ECHO, port $PEER_A_PORT"
RUST_LOG=zenoh=debug "$BIN" queryable "$KEY_ECHO" --payload "$PAYLOAD_ECHO" --config "$CFG_A" > "$A_OUT" 2>&1 &
A_PID=$!
BG_PIDS+=("$A_PID")

log "waiting 2s for Peer A to initialize"
sleep 2
if kill -0 "$A_PID" 2>/dev/null; then
  ok "Peer A running (pid=$A_PID)"
else
  bad "Peer A exited early"
  echo "--- peer-a.log ---"
  cat "$A_OUT"
  exit 1
fi

# ---- Run Peer B (TCP-only get) ----
B_OUT="$TMP/peer-b.log"
log "running Peer B (TCP-only) get on $KEY_ECHO --connect tcp/127.0.0.1:${PEER_A_PORT}"
RUST_LOG=zenoh=debug "$BIN" get "$KEY_ECHO" \
  --no-quic \
  --config "$CFG_B" \
  --connect "tcp/127.0.0.1:${PEER_A_PORT}" \
  --timeout-ms 5000 \
  > "$B_OUT" 2>&1
PEER_B_EXIT=$?

# ---- Assertions ----
echo
log "--- Assertions ---"

# (a) Peer A log contains QUIC listener evidence
if grep -qiE 'quic.*(listener|listen|accept|bound)|Accepting quic' "$A_OUT" 2>/dev/null; then
  ok "(a) Peer A QUIC listener evidence found in log"
else
  bad "(a) Peer A QUIC listener evidence NOT found in log"
  echo "    (searched: 'quic.*(listener|listen|accept|bound)|Accepting quic')"
  echo "    --- peer-a.log tail ---"
  tail -30 "$A_OUT"
fi

# (b) Peer B exited 0
if [[ $PEER_B_EXIT -eq 0 ]]; then
  ok "(b) Peer B get returned exit code 0"
else
  bad "(b) Peer B get returned exit code $PEER_B_EXIT"
  echo "--- peer-b.log ---"
  cat "$B_OUT"
fi

# (c) Peer B output contains reply payload
if grep -q "$PAYLOAD_ECHO" "$B_OUT" 2>/dev/null; then
  ok "(c) Peer B received '$PAYLOAD_ECHO'"
else
  bad "(c) Peer B did NOT receive '$PAYLOAD_ECHO'"
  echo "--- peer-b.log ---"
  cat "$B_OUT"
fi

# (d) Peer B log contains TCP link evidence (WARNING only — log format may vary)
# TODO: tighten assertion when zenoh log format stabilizes
if grep -qiE 'tcp/|TcpLink|TcpLinkManager' "$B_OUT" 2>/dev/null; then
  ok "(d) Peer B TCP link evidence found in log"
else
  warn "(d) Peer B TCP link evidence NOT found (soft check — zenoh log format may not emit this)"
  warn "    searched: 'tcp/|TcpLink|TcpLinkManager'"
fi

# ---- Shutdown ----
echo
log "shutting down Peer A"
kill "$A_PID" 2>/dev/null || true
sleep 0.5
if kill -0 "$A_PID" 2>/dev/null; then
  bad "Peer A did not exit on SIGTERM"
else
  ok "Peer A exited cleanly"
fi

echo
if [[ $fail -eq 0 ]]; then
  printf "\033[32mQUIC FALLBACK TEST PASSED\033[0m\n"
  exit 0
else
  printf "\033[31mQUIC FALLBACK TEST FAILED\033[0m\n"
  if [[ "${STYLOS_KEEP_LOGS:-0}" == "1" ]]; then
    printf "logs at %s\n" "$TMP"
  fi
  exit 1
fi
