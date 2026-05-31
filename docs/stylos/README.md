# Stylos Documentation

Zenoh-based interconnect foundation for the workspace. See the workspace-level
[PRD-019](../prd/prd-019-stylos-foundation.md) for design intent and scope.

> **Stylos core now lives in its own repo: [github.com/tasanakorn/stylos](https://github.com/tasanakorn/stylos).**
> It was extracted from this monorepo (formerly `apps/stylos/`) to be a shared
> standard. That repo holds the `stylos` library crate, the `stylos-cli`, the Go
> sidecar, and the cross-language interop scripts. `stele-server` consumes the
> library as a pinned git dependency (currently tag `v0.2.1`) — see
> `apps/stele/crates/stele-server/Cargo.toml`. The docs in this folder are the
> design specs and remain the canonical reference for the protocol.

## Contents

| Doc                                       | What                                                                |
| ----------------------------------------- | ------------------------------------------------------------------- |
| [architecture.md](architecture.md)        | Process model, crate split, config construction, data flow          |
| [addressing.md](addressing.md)            | `stylos/<realm>/<role>/<instance>` key-expr grammar + POC keys      |
| [discovery.md](discovery.md)              | Multicast scouting, gossip, data-listener layout, failure modes     |
| [poc.md](poc.md)                          | POC scenarios, acceptance criteria, smoke-test runbook              |
| [cross-lang.md](cross-lang.md)            | Rust/Go/Python/TS binding status, CGO prereq, wire compatibility    |
| [research-b0.md](research-b0.md)          | Primary-source zenoh 1.x API findings; durable implementation input |

Binaries, crate sources, example config, and dev-cert script live in the
external [stylos repo](https://github.com/tasanakorn/stylos) (they were under
`apps/stylos/` before the extraction).

## Consumers

- **stele-server (v0.17.0+)** embeds a stylos peer via [PRD-022](../prd/prd-022-stylos-in-stele-server.md). It defaults to `mode = "router"` so the always-on stele process provides a stable discovery hub for short-lived peers.
