# Stylos

Stylos is the workspace's zenoh-based interconnect foundation — a
cross-process, cross-host, cross-language signal layer that Stele, Steop,
and future tooling will eventually ride on.

Status: **v0.1.0.** Rust peer + CLI fully implemented (pub/sub/get/queryable/identity). Go peer with `get` + `pub` subcommands; `sub`/`queryable` Go-side TODO.

See [`docs/stylos/`](../../docs/stylos/README.md) and
[PRD-019](../../docs/prd/prd-019-stylos-foundation.md).

## Build

Rust peer:

```bash
cd apps/stylos
cargo build -p stylos-cli        # produces ./target/debug/stylos
```

Go peer (requires a local `zenoh-c` build — see
[docs/stylos/cross-lang.md](../../docs/stylos/cross-lang.md) for the one-time
cmake flow under `third_party/zenoh-c-prefix/`):

```bash
cd apps/stylos/go
./build.sh                       # produces ./target/stylos (Go binary)
```

## Config

Stylos reads JSON5 from (first match wins):

1. `--config <path>` flag
2. `$STYLOS_CONFIG`
3. `./stylos.json5`
4. Built-in defaults (realm=dev, role=cli, instance=cli-`<ts>`)

Start from `stylos.example.json5`. No TLS/cert story at v0.1.0 — the data
plane is UDP + TCP on port 31747.

## Smoke tests (scripted)

Fastest path — three scripts under `./scripts/`:

```bash
./scripts/smoke-test.sh             # Rust↔Rust pub/sub/get/queryable
./scripts/go-interop-test.sh        # Rust queryable ← Go get
./scripts/go-pub-rust-sub-test.sh   # Rust sub ← Go pub
```

All three pin ports and `--connect` explicitly, so they sidestep the macOS
multicast-loopback caveat below. Together they cover every PRD-019 §4.8.4
acceptance criterion.

## Two-terminal smoke test (same host, LAN multicast)

Terminal A — queryable + subscriber:

```bash
./target/debug/stylos queryable stylos/dev/poc/echo --payload reply-from-rust &
./target/debug/stylos sub stylos/dev/poc/rust
```

Terminal B — pub + get:

```bash
./target/debug/stylos pub stylos/dev/poc/rust "hello-from-rust"
./target/debug/stylos get stylos/dev/poc/echo --timeout-ms 3000
```

Expected: Terminal A's `sub` prints `stylos/dev/poc/rust hello-from-rust`;
Terminal B's `get` prints `stylos/dev/poc/echo reply-from-rust`.

## Subcommands

| Command                            | Behaviour                                        |
| ---------------------------------- | ------------------------------------------------ |
| `stylos pub <KE> <msg>`            | Publish one sample, exit                         |
| `stylos sub <KE>`                  | Subscribe; print samples until Ctrl-C            |
| `stylos get <KE> [--timeout-ms N]` | Query; print replies until deadline              |
| `stylos queryable <KE> [--payload]`| Serve replies until Ctrl-C                       |
| `stylos identity`                  | Print resolved identity + root key, exit         |

Global flags (valid on every subcommand): `--config <path>`,
`--connect <endpoint>` (repeatable, REPLACES config).

## macOS multicast loopback caveat

On macOS, two processes on the same host can miss each other's multicast
announcements depending on interface resolution. If discovery fails,
force a direct TCP connect from one side:
`stylos sub stylos/dev/poc/rust --connect tcp/127.0.0.1:31747`.
