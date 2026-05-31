# Stylos Cross-Language Notes

Stylos peers can be written in any language with a zenoh binding. PRD-019 §4.7 captures the initial scope.

> **Paths below are now relative to the external [stylos repo](https://github.com/tasanakorn/stylos), not this monorepo.** The Go sidecar, `third_party/`, and build scripts moved there in the extraction — drop the `apps/stylos/` prefix (e.g. `apps/stylos/go/build.sh` → `go/build.sh` inside a stylos checkout).

## Binding status

| Language   | Status at v0.1.0    | Binding                            | Notes                                                                                                                                     |
| ---------- | ------------------- | ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Rust       | **Implemented**     | `zenoh` crate (pinned 1.9.0)       | Pass B-Rust. Primary reference implementation.                                                                                            |
| Go         | **Implemented** (v0.1.0, `get` + `pub`; `sub`/`queryable` TODO) | `eclipse-zenoh/zenoh-go` (v1.9.0)  | CGO over `zenoh-c` 1.9.0. Requires a local zenoh-c build — see below.                |
| Python     | Nice-to-have        | `zenoh-python` (PyO3)              | Native, full-featured; lowest-risk addition. Not in v0.1.0.                                                                               |
| TypeScript | Nice-to-have (open) | `zenoh-ts`                         | Historically WebSocket-via-router only; current state unverified (PRD §9 Q2).                                                             |
| C / C++    | Not in scope        | —                                  | Could follow easily via `zenoh-c` / `zenoh-cpp`. Deferred.                                                                                |

## Go (zenoh-c prerequisite)

`zenoh-go` v1.9.0 is a CGO wrapper over `zenoh-c`, not a pure-Go rewrite. Building the Go peer requires a locally-built `zenoh-c` 1.9.0 with the unstable API enabled. The build is reproducible from the stylos tree — nothing is shipped in the repo.

**Layout (both gitignored):**

- `apps/stylos/third_party/zenoh-c/` — clone of `eclipse-zenoh/zenoh-c` at tag `1.9.0`.
- `apps/stylos/third_party/zenoh-c-prefix/` — install prefix populated by `cmake --install`.

**One-time build (~5 min; incremental rebuilds are fast):**

```bash
cd apps/stylos/third_party
git clone --depth 1 --branch 1.9.0 https://github.com/eclipse-zenoh/zenoh-c.git
cmake -S zenoh-c -B zenoh-c/build \
  -DZENOHC_BUILD_WITH_UNSTABLE_API=ON \
  -DCMAKE_INSTALL_PREFIX="$PWD/zenoh-c-prefix" \
  -DCMAKE_BUILD_TYPE=Release
cmake --build zenoh-c/build --parallel
cmake --install zenoh-c/build
```

**Go-side wrapper:** `apps/stylos/go/build.sh` points `PKG_CONFIG_PATH` / `CGO_CFLAGS` / `CGO_LDFLAGS` at `third_party/zenoh-c-prefix/` and invokes `go build -o target/stylos`. Contributors run `./build.sh` from `apps/stylos/go/`; no global env required.

**Host prereqs:** `cmake`, `git`, `openssl`, and a Rust toolchain (rustup pulls one in automatically during the zenoh-c build).

## Wire compatibility

All zenoh bindings share the underlying protocol and default serialization. A Rust stylos peer talking to a Go stylos peer is wire-compatible as long as both sides pin a mutually-compatible zenoh version. Pass B pinned Rust at `=1.9.0`; Pass B-Go pins `zenoh-go` at `v1.9.0` against `zenoh-c 1.9.0`.

Key expressions, selectors, and multicast scouting are protocol-level — not binding-specific. A Python observer can `stylos sub stylos/dev/**` without needing any stylos-specific protocol knowledge.

## Background

Primary-source API verification (zenoh 1.9.0 Rust + zenoh-go 1.9.0) lives in [research-b0.md](research-b0.md). It captures the answers to PRD-019 §9 Q1, Q3, Q4 and surfaces the CGO constraint above.
