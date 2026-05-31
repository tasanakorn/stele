# Stylos POC — Rust ↔ Go pub/sub/get/queryable

Exercises all four zenoh interaction primitives across stylos peers. Spec: PRD-019 §4.8.

> **The scripts and binaries below now live in the external [stylos repo](https://github.com/tasanakorn/stylos), not this monorepo.** Run them from a stylos checkout — drop the `apps/stylos/` prefix (e.g. `cd apps/stylos` → the stylos repo root, `apps/stylos/scripts/` → `scripts/`).

## Status

| Primitive         |  Rust ↔ Rust  |     Rust ↔ Go    | Notes                                                                    |
| ----------------- | :-----------: | :--------------: | ------------------------------------------------------------------------ |
| pub/sub           |      ✅        |    ✅ (Go→Rust)  | Go `pub` → Rust `sub` exercised; Rust `pub` → Go `sub` pending Go `sub`. |
| get/queryable     |      ✅        |    ✅ (Go→Rust)  | Go `get` → Rust `queryable` exercised; reverse pending Go `queryable`.   |

Go peer shipped in v0.1.0 with `get` and `pub` subcommands; `sub` and `queryable` Go-side remain TODO. See [cross-lang.md](cross-lang.md).

## Key expressions

```
stylos/dev/poc/rust   # Rust peer publishes
stylos/dev/poc/go     # Go peer publishes (Pass B-Go)
stylos/dev/poc/echo   # Queryable endpoint; either peer can get
```

## Shipped smoke tests

Four scripts under `apps/stylos/scripts/` exercise the acceptance matrix. Each pins ports and uses explicit `--connect tcp/127.0.0.1:<port>` to sidestep macOS multicast-loopback flakiness.

| Script                      | Validates                                                       | PRD-019 §4.8.4 criteria |
| --------------------------- | --------------------------------------------------------------- | ----------------------- |
| `smoke-test.sh`             | Rust↔Rust pub/sub + get/queryable + clean SIGTERM shutdown      | 1, 3, 5, 6              |
| `go-interop-test.sh`        | Rust `queryable` ← Go `get` (cross-lang reply direction)        | cross-lang get path     |
| `go-pub-rust-sub-test.sh`   | Rust `sub` ← Go `pub` (cross-lang publish direction)            | 2                       |

Run any of them directly after building both peers:

```bash
cd apps/stylos
cargo build -p stylos-cli
(cd go && ./build.sh)            # Go binary at apps/stylos/go/target/stylos
./scripts/smoke-test.sh
./scripts/go-interop-test.sh
./scripts/go-pub-rust-sub-test.sh
```

Each script prints green `PASS` lines and ends with a summary banner. All six §4.8.4 acceptance criteria pass via the four scripts combined.

## Scenario — two hosts on a LAN

Pure multicast discovery, no explicit `--connect`. Run `stylos queryable stylos/dev/poc/echo` on host A and `stylos get stylos/dev/poc/echo` on host B. Both hosts must be on the same broadcast domain and multicast must not be filtered. Outcome pending — no two-host bed has been exercised yet.

## macOS caveat

Same-host multicast loopback is unreliable on macOS: two stylos processes on one host may fail to discover each other via scouting alone, even though the mesh works fine cross-host. The smoke-test script avoids this with pinned ports + `--connect`. Document-level acceptance of the caveat lives in [discovery.md](discovery.md).
