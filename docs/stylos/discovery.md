# Stylos Discovery

Zenoh calls this mechanism **scouting**. Stylos docs say "discovery" in
prose; the config keys are spelled `scouting.*`.

## LAN multicast

- Group: `224.0.0.224:31746` (zenoh default port is 7446; stylos overrides
  to 31746 to stay off stock deployments).
- `scouting.multicast.enabled = true` — peers announce on start and listen
  for announcements.
- `scouting.gossip.enabled = true` — once two peers meet, remaining peers
  learn by propagation without needing a full address book.

## Data listeners

After discovery, peers connect over the advertised data locators. Stylos
picks a free port in `[31747, 31747 + 8)` by dual-binding TCP and UDP.
Both listeners are advertised on the same port number:

```
quic/0.0.0.0:31747
tcp/0.0.0.0:31747
```

QUIC is tried first; TCP is the fallback when QUIC's UDP handshake fails
or is blocked. With `--no-quic`, a peer drops the `quic/...` locator
entirely and accepts TCP only.

## Non-multicast networks (VPN, tailnet, WAN)

Out of scope at v0.1.0. Supply explicit `connect.endpoints` in the config
or via `--connect tcp/host:port`; the peer dials those directly. Cross-LAN
bridging via a router peer is a follow-up PRD.

## Failure modes

| Scenario                         | Behaviour                                                         |
| -------------------------------- | ----------------------------------------------------------------- |
| Multicast blocked on network     | Peers never meet; user must provide `--connect`.                  |
| QUIC TLS cert missing/expired    | QUIC listener fails to bind; stylos continues with TCP only.      |
| Port 31747 already bound         | Walk forward (cap 8); log the chosen port; fail if all taken.     |
