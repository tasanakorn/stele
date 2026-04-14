# Stylos Documentation

Zenoh-based interconnect foundation for the workspace. See the workspace-level
[PRD-019](../prd/prd-019-stylos-foundation.md) for design intent and scope.

## Contents

| Doc                                       | What                                                                |
| ----------------------------------------- | ------------------------------------------------------------------- |
| [architecture.md](architecture.md)        | Process model, crate split, config construction, data flow          |
| [addressing.md](addressing.md)            | `stylos/<realm>/<role>/<instance>` key-expr grammar + POC keys      |
| [discovery.md](discovery.md)              | Multicast scouting, gossip, data-listener layout, failure modes     |
| [poc.md](poc.md)                          | POC scenarios, acceptance criteria, smoke-test runbook              |
| [cross-lang.md](cross-lang.md)            | Rust/Go/Python/TS binding status, CGO prereq, wire compatibility    |
| [research-b0.md](research-b0.md)          | Primary-source zenoh 1.x API findings; durable implementation input |

Binaries, crate sources, example config, and dev-cert script live under
[`apps/stylos/`](../../apps/stylos/README.md).
