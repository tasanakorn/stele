# PRD-022 — Stylos integration in stele-server

- **Status:** Implemented (v0.17.0)
- **Target version:** workspace v0.17.0 (stylos crates stay at 0.1.0; this PRD does not touch them)
- **Scope:** `apps/stele/crates/stele-server/` (new `stylos_session` module, config, CLI, feature flag, tray wiring, health endpoint), `apps/stele/Dockerfile` (build context change), `docs/stele/server.md`, `docs/stele/deployment.md`, `docs/stele/http-api.md`, `docs/stylos/README.md`, `docs/README.md`, lock-step version bumps (`apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`)
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Embed a zenoh peer inside `stele-server`** via `stylos_session::open_session`, so that the always-on stele process joins the LAN stylos mesh on startup. Works in **both** the desktop (tray) and headless (Linux/Docker) builds.
2. **Default to router mode.** Because stele-server is the most stable long-lived process on the network, it becomes the natural zenoh router — client-only peers (future steop, stele-cli, or third-party tooling) can attach without needing multicast. Peer-only mode is available as an opt-out.
3. **Clean lifecycle.** The zenoh session is opened once, just before the axum listener, and survives `STELE_BIND` rebinds. Shutdown is tied to the existing `CancellationToken` path (tray Quit on desktop, SIGINT/SIGTERM on headless). `session.close().await` runs before the process exits.
4. **Observable status.** A new `GET /api/v1/health` endpoint reports stylos state (`enabled`, `mode`, `zid`, peer count, router count, listen endpoints) alongside server liveness. The macOS tray gains one new status row showing the stylos session summary.
5. **Prove the wiring end-to-end.** Stele-server publishes a 5-second heartbeat on `stylos/<realm>/stele/<instance>/heartbeat` and registers a queryable on `stylos/<realm>/stele/<instance>/info` that returns a JSON blob. The POC is verified by `stylos sub` / `stylos get` from `apps/stylos/` on another host (and, per the macOS multicast caveat, on the same host using explicit `--connect tcp/127.0.0.1:<port>`).

## 2. Non-goals

- **No migration of mailbox, notify, or MCP off HTTP onto zenoh.** Those surfaces keep their existing REST/MCP paths at v0.17.0. A follow-up PRD may propose pub/sub-backed mailbox delivery; this PRD does not.
- **No auth, ACL, or TLS hardening beyond what stylos 0.1.0 already does.** Stylos drops QUIC silently when no TLS certs are configured (documented behavior); stele-server inherits that default and does not ship its own cert story yet.
- **No steop or stele-cli wiring as peers.** Steop still talks HTTP to stele-server; `stele` CLI is unchanged. The only zenoh peer in v0.17.0 is stele-server itself.
- **No payload schemas beyond opaque bytes and one JSON info blob.** Heartbeat sends the literal bytes `b"alive"`; the info queryable returns a fixed JSON shape (see §4.7). No protobuf / CBOR / versioned schema yet.
- **No new stylos crate work.** Router mode is a config toggle in the existing `ZenohSection`; no changes land in `apps/stylos/crates/**` for this PRD.
- **No automated test harness.** Same manual-smoke posture as the rest of the workspace.

## 3. Background & Motivation

### 3.1 Current state

Stele-server is the always-on process at the center of the workspace (see [docs/architecture.md](../architecture.md)):

- Two binaries (`stele-server`, `stele`), one SQLite database, axum + rmcp on the same port.
- Desktop build: macOS tray app (`cargo run -p stele-server`), headless build: Linux daemon / Docker image (`--features headless --no-default-features`).
- Entry point `apps/stele/crates/stele-server/src/main.rs` selects between the two via the `desktop` feature, both call into `run_server(config, pool, ct, bind_state, auth_state)` in `src/server.rs`.
- Config is read via `apps/stele/crates/stele-server/src/settings.rs` (a `toml::Table` load that preserves unknown keys) and merged with CLI flags from `clap`.

As of v0.16.1 there is no persistent connection between stele-server and any other host process — everything is request/response over HTTP/MCP. There is also no `/health` endpoint; the closest surface is `GET /api/v1/stats`.

[PRD-019](prd-019-stylos-foundation.md) landed `apps/stylos/` as an independent Cargo workspace with five reusable library crates (`stylos-common`, `stylos-identity`, `stylos-config`, `stylos-transport`, `stylos-session`) plus a `stylos` CLI, pinned at `zenoh = "=1.9.0"`. The session factory is a single call:

```rust
let session: Arc<zenoh::Session> = stylos_session::open_session(&cfg, &Default::default()).await?;
```

Nothing in the workspace consumes those libraries yet.

### 3.2 Why integrate stylos into stele-server now

- **Stele-server is the natural router.** It is the only process guaranteed to be running and reachable on a developer's machine. Every other peer (steop, stele-cli, future watchers) is short-lived or host-local. Putting a long-lived router on the same process that already hosts SQLite and axum gives the mesh a stable hub without adding a new daemon.
- **Prove the wiring under realistic conditions.** Running `stylos` as a standalone CLI verifies the crates in isolation; embedding the same libraries in a production-shape process (tray app, systemd unit, Docker image) surfaces lifecycle, feature-gating, and deployment issues that the CLI cannot.
- **Set the addressing precedent.** Every future stele-consumer of the mesh will key off `stylos/<realm>/stele/<instance>/*`. Locking that convention in this PRD means steop, watchers, and any third-party tooling inherit a canonical namespace.

### 3.3 Why default-on, router-mode

- **Default-on** keeps the user experience consistent between desktop and headless builds — if you run stele-server, you join the mesh. Opting out is a cargo-feature composition change (see §4.5).
- **Router mode** is a zero-code toggle in zenoh (`ZenohSection::mode = "router"`), and it strictly adds capability: a router still accepts peer-mode connections but additionally accepts client-mode attachments and forwards across discovery boundaries. Nothing a v0.17.0 user would want is lost by choosing router over peer.

## 4. Design

### 4.1 Canonical key-expr convention

All stele-server stylos traffic sits under:

```
stylos/<realm>/stele/<instance>/<leaf>
```

| Chunk        | Example           | Notes                                                                                          |
| ------------ | ----------------- | ---------------------------------------------------------------------------------------------- |
| `stylos`    | literal           | Root namespace (PRD-019, unchanged)                                                            |
| `<realm>`    | `dev`             | From `[stylos].realm`. Default `"dev"`                                                         |
| `stele`     | literal           | Role — `stele-server` is always the `stele` role in the stylos grammar                         |
| `<instance>` | `dev-mbp`         | Per-process id derived from hostname (§4.2). Operator-overridable                              |
| `<leaf>`     | `heartbeat`, `info` | Reserved leaves in v0.17.0; more may be added in later PRDs                                  |

Two leaves ship at v0.17.0:

- `stylos/<realm>/stele/<instance>/heartbeat` — publisher, emits every 5 s.
- `stylos/<realm>/stele/<instance>/info` — queryable, returns a JSON blob on demand.

The original design brief's `stylos/stele/<host>/info` shape violates PRD-019 addressing (`stele` is the role, not the realm). The canonical 5-chunk form above is load-bearing for this PRD and every future stele-server consumer of stylos.

### 4.2 `<instance>` derivation rule

In priority order:

1. **Operator override:** if `[stylos].instance` is set in config or `STELE_STYLOS_INSTANCE` env var is non-empty, use that verbatim after grammar validation (`[a-z0-9][a-z0-9-]*`). Reject on mismatch.
2. **Normalized hostname:** read `hostname::get()` (crate `hostname`), lowercase it, replace every non-`[a-z0-9-]` character with `-`, strip leading/trailing dashes, trim to 32 chars. If the result is non-empty and matches the grammar, use it.
3. **Fallback:** `stele-<short-zid>` where `<short-zid>` is the first 8 hex chars of `session.info().zid().await.to_string()`. This is only reachable if hostname normalization yields an empty string — practically never in production, but the fallback removes one panic path.

The chosen instance is logged at startup (`tracing::info!`) and surfaced on both `/api/v1/health` and the tray status row.

### 4.3 Config schema

A new `[stylos]` section is added to stele-server's `config.toml`. Full example:

```toml
# ~/Library/Application Support/Stele/config.toml (desktop)
# or the path passed via --config / STELE_CONFIG (headless)

bind     = "0.0.0.0:3100"
db       = "/var/lib/stele/stele.db"
mcp_path = "/mcp"
auth_key = "…"

[stylos]
enabled  = true          # default: true. Set false to skip session open.
mode     = "router"      # default: "router". Alt: "peer" | "client".
realm    = "dev"         # default: "dev".
instance = "dev-mbp"     # optional; auto-derived from hostname if omitted (§4.2).

# Connect endpoints — empty on pure-LAN multicast, populated on WAN/tailnet.
# Pass-through to zenoh connect.endpoints.
connect = ["tcp/10.0.0.5:31747"]

# Optional: disable QUIC even when certs are present (TCP-only).
no_quic = false

# Optional: override listen port range (defaults to stylos defaults — 31747 + walk).
# Leave unset in normal deployments.
# listen_port_start = 31747
```

All fields are optional; sane defaults kick in when the section is absent. The section being missing entirely is equivalent to `enabled = false` **only if** the cargo `stylos` feature is off; when the feature is on the default is `enabled = true` with `mode = "router"` and `realm = "dev"`.

Loading strategy (see §9 Q1): stele-server declares its own TOML-friendly `StyloSettings` struct in `src/config.rs` that mirrors the subset of `stylos_config::StylosConfig` we expose. The struct is then translated into a `stylos_config::StylosConfig` right before the `open_session` call. Rationale: avoids pulling `json5` (a transitive dep of `stylos-config` via its loader path) into stele-server, and keeps the TOML schema stable even if upstream stylos reshuffles its own config layout.

### 4.4 Env vars and CLI flags

> **Superseded by [PRD-023](prd-023-stylos-default-udp.md) (v0.18.0).** Stylos now defaults to UDP + TCP on port 31747; the QUIC listener, TLS cert story, and `no_quic` override are removed.

All `[stylos]` fields are overridable via CLI flags and env vars, mirroring the existing clap-driven pattern in `stele-server`:

| CLI flag              | Env var                   | Type               | Maps to                 |
| --------------------- | ------------------------- | ------------------ | ----------------------- |
| `--stylos`           | `STELE_STYLOS_ENABLED`    | bool               | `[stylos].enabled`     |
| `--no-stylos`        | —                         | flag               | `[stylos].enabled=false` |
| `--stylos-mode`      | `STELE_STYLOS_MODE`       | `peer\|router\|client` | `[stylos].mode`     |
| `--stylos-realm`     | `STELE_STYLOS_REALM`      | string             | `[stylos].realm`       |
| `--stylos-instance`  | `STELE_STYLOS_INSTANCE`   | string             | `[stylos].instance`    |
| `--stylos-connect`   | `STELE_STYLOS_CONNECT`    | comma-separated    | `[stylos].connect`     |
| `--stylos-no-quic`   | `STELE_STYLOS_NO_QUIC`    | bool               | `[stylos].no_quic`     |

Precedence: CLI flag > env var > config.toml > built-in default.

### 4.5 Cargo feature composition

Stele-server already selects `desktop` vs `headless` via mutually-named features:

```toml
[features]
default  = ["desktop"]
desktop  = ["tray-icon", "muda", "image", "dirs", "winit", "eframe", "notify-rust", "arboard"]
headless = []
```

Proposed additions:

```toml
[features]
default  = ["desktop"]
desktop  = ["tray-icon", ..., "stylos"]
headless = ["stylos"]

# New internal feature: toggles the zenoh + stylos deps.
stylos = [
    "dep:stylos-session",
    "dep:stylos-config",
    "dep:stylos-common",
    "dep:stylos-identity",
    "dep:zenoh",
    "dep:hostname",
]

[dependencies]
stylos-session  = { path = "../../../stylos/crates/stylos-session",  optional = true }
stylos-config   = { path = "../../../stylos/crates/stylos-config",   optional = true }
stylos-common   = { path = "../../../stylos/crates/stylos-common",   optional = true }
stylos-identity = { path = "../../../stylos/crates/stylos-identity", optional = true }
zenoh           = { version = "=1.9.0", optional = true }
hostname        = { version = "0.4",    optional = true }
```

Both top-level feature sets (`desktop`, `headless`) include `stylos`, so every default build joins the mesh. A headless distro that wants to drop zenoh can explicitly build with `--no-default-features --features "headless-no-stylos"` (introduced in this PRD as a third feature that mirrors `headless` minus stylos — naming TBD in §9 Q4, current proposal: the explicit opt-out path is `--no-default-features --features headless` with `stylos` removed from the `headless` feature instead — see §9 Q4 for the final decision).

### 4.6 Cross-workspace dependency strategy

`apps/stele/Cargo.toml` and `apps/stylos/Cargo.toml` are two independent Cargo workspace roots. PRD-019 (§4.6) deliberately locked that independence. Three candidate strategies were considered:

1. **Path deps across workspaces** (recommended). `stylos-* = { path = "../../../stylos/crates/stylos-*" }` from `stele-server/Cargo.toml`. Compiles locally without any changes to either workspace manifest. Requires one change to `apps/stele/Dockerfile`: the build context must be the repo root (not `apps/stele/`) so that `apps/stylos/` is copyable into the build stage.
2. **Nest stylos crates under `apps/stele/crates/stylos-*`** — duplicates files or requires a multi-root workspace. Violates PRD-019's independence decision; rejected.
3. **Publish stylos crates to a git or crates.io registry** — cleanest long-term but adds release ceremony before v0.17.0 can ship. Deferred to a follow-up PRD when stylos stabilizes.

This PRD commits to strategy **#1**. Dockerfile changes:

- Build context: `docker build -f apps/stele/Dockerfile .` from repo root (instead of `docker build apps/stele/`).
- Inside the Dockerfile: `COPY apps/stele/ /build/stele/` **and** `COPY apps/stylos/Cargo.toml apps/stylos/Cargo.lock /build/stylos/` + `COPY apps/stylos/crates/ /build/stylos/crates/`.
- Working dir for cargo build: `/build/stele/`.
- `.dockerignore` at repo root is added to keep the image lean.

### 4.7 Lifecycle: where the session lives

In `apps/stele/crates/stele-server/src/server.rs::run_server`, the session is opened **after** `notify::init()` and **before** the `STELE_BIND` rebind loop:

```rust
#[cfg(feature = "stylos")]
let stylos_handle = stylos_session::spawn(&config.stylos, ct.clone()).await?;
//                                                                  ^-- new module under src/stylos_session.rs

// …existing axum rebind loop…

// On shutdown:
#[cfg(feature = "stylos")]
stylos_handle.shutdown().await;
```

The `stylos_session` module owns:

- `Arc<zenoh::Session>` (exposed via `StylosHandle::session()` for the axum router state to read identity info).
- A long-lived task that publishes `b"alive"` to `stylos/<realm>/stele/<instance>/heartbeat` every 5 s.
- A long-lived `Queryable` on `stylos/<realm>/stele/<instance>/info` that responds with a JSON blob (schema below).
- A shutdown path that cancels the heartbeat task, drops the queryable, and calls `session.close().await`.

The session **must not** be restarted on `STELE_BIND` rebinds. Axum rebinds change only the HTTP listener; the zenoh session's ZID and transport identity are stable across rebinds.

### 4.8 Heartbeat payload

- Key: `stylos/<realm>/stele/<instance>/heartbeat`
- Period: **5 s** (tokio `interval`, tick-skip on lag).
- Payload: literal bytes `b"alive"`. No JSON, no timestamp — subscribers that want freshness use the sample's zenoh-side timestamp.
- Encoding: `zenoh::bytes::Encoding::APPLICATION_OCTET_STREAM`.
- Publisher config: `CongestionControl::Drop`, `Priority::Data`, `Reliability::BestEffort`. A missed heartbeat is not a fault.

### 4.9 Info queryable payload

> **Superseded by [PRD-023](prd-023-stylos-default-udp.md) (v0.18.0).** Stylos now defaults to UDP + TCP on port 31747; the QUIC listener, TLS cert story, and `no_quic` override are removed.

- Key: `stylos/<realm>/stele/<instance>/info`
- Behavior: on every query, build a `StyloInfo` struct and respond with a single sample containing the JSON body.
- Schema:

```json
{
  "zid": "e0a1c9…",
  "mode": "router",
  "realm": "dev",
  "instance": "dev-mbp",
  "version": "0.17.0",
  "stylos_version": "0.1.0",
  "listen_endpoints": ["tcp/0.0.0.0:31747", "quic/0.0.0.0:31747"],
  "started_at": "2026-04-14T12:34:56Z"
}
```

- Encoding: `Encoding::APPLICATION_JSON`.
- Source: `zid` and `listen_endpoints` come from `session.info()`; `version` is `env!("CARGO_PKG_VERSION")`; `stylos_version` is pinned to `0.1.0` via a new constant in `stylos-common` (if absent, hard-coded string for v0.17.0).

### 4.10 Health endpoint

New route: `GET /api/v1/health`, mounted on the same `api::router` that hosts `/api/v1/stats`. Requires no auth (same posture as `/api/v1/stats`; auth-gated routes are declared elsewhere).

Response body:

```json
{
  "status": "ok",
  "version": "0.17.0",
  "db_ok": true,
  "stylos": {
    "enabled": true,
    "mode": "router",
    "zid": "e0a1c9…",
    "realm": "dev",
    "instance": "dev-mbp",
    "listen_endpoints": ["tcp/0.0.0.0:31747"],
    "peers": 2,
    "routers": 0
  }
}
```

When the `stylos` feature is off, the `stylos` field is omitted (not `null`) so the JSON shape matches the runtime capability. When the feature is on but `enabled = false`, `stylos` is `{ "enabled": false }`.

Wiring: the axum router state grows from `DbPool` to a small struct (`ApiState { db: DbPool, stylos: Option<Arc<zenoh::Session>> }`). Every existing handler signature changes from `State(db)` to `State(state): State<ApiState>` and derives `db = state.db` inline. The state clone is cheap (`Arc` internally).

### 4.11 Tray status row (desktop only)

In `apps/stele/crates/stele-server/src/tray.rs` (around line 245–288, near the existing server-status `MenuItem`), add one new disabled menu item:

```
Stylos: router · e0a1c9 · 2 peers
```

Updated in the tray's `about_to_wait` tick by reading the `Arc<zenoh::Session>` handed to `tray::run` alongside `bind_state`. When the feature is off, the row is omitted entirely (not shown as "disabled").

## 5. Changes by Component

| Component                                    | Change                                                                                                     | Files                                                                                                                                  |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| stele-server Cargo manifest                 | Add `stylos` feature, optional stylos/zenoh deps, propagate through `desktop` + `headless`.                 | `apps/stele/crates/stele-server/Cargo.toml`                                                                                            |
| stele-server config                         | New `StyloSettings` struct, `[stylos]` TOML section, CLI flags, env vars.                                  | `apps/stele/crates/stele-server/src/config.rs`, `apps/stele/crates/stele-server/src/settings.rs`, `apps/stele/crates/stele-server/src/main.rs` |
| stele-server session module                 | New `stylos_session.rs` owning `Arc<zenoh::Session>`, heartbeat task, info queryable, shutdown.              | `apps/stele/crates/stele-server/src/stylos_session.rs` (new), `apps/stele/crates/stele-server/src/server.rs`                            |
| stele-server axum state                     | Router state becomes `ApiState { db, stylos }`; `/api/v1/health` route added.                               | `apps/stele/crates/stele-server/src/api.rs` (and any handler that currently takes `State<DbPool>`)                                      |
| stele-server tray (desktop)                 | Stylos status row in the menu.                                                                              | `apps/stele/crates/stele-server/src/tray.rs`                                                                                            |
| Dockerfile                                   | Build context = repo root; copy `apps/stele/` and `apps/stylos/` into the build stage.                      | `apps/stele/Dockerfile`, new `/.dockerignore`                                                                                           |
| Docs — server internals                     | New section on the stylos session lifecycle, feature gating, config surface.                                | `docs/stele/server.md`                                                                                                                  |
| Docs — deployment                           | Add `STELE_STYLOS_*` env vars; document repo-root Docker build context change.                              | `docs/stele/deployment.md`                                                                                                              |
| Docs — HTTP API                             | New `/api/v1/health` endpoint reference.                                                                    | `docs/stele/http-api.md`                                                                                                                |
| Docs — stylos subtree                       | One-line "Consumers" note linking to this PRD.                                                              | `docs/stylos/README.md`                                                                                                                 |
| Docs — PRD index                             | Add PRD-022 row after PRD-021.                                                                              | `docs/README.md`                                                                                                                        |
| Version bump                                 | Lock-step `0.16.1 → 0.17.0` for workspace + stele + steop plugins + steop Go binary.                        | `apps/stele/Cargo.toml`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`, `apps/steop/version.go` |

No changes to `apps/stylos/`, `apps/steop/`, `apps/stele/crates/stele-common/`, or `apps/stele/crates/stele-cli/`.

## 6. Edge Cases

> **Superseded by [PRD-023](prd-023-stylos-default-udp.md) (v0.18.0).** Stylos now defaults to UDP + TCP on port 31747; the QUIC listener, TLS cert story, and `no_quic` override are removed.

| Scenario                                                                                     | Behavior                                                                                                                                                                                |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Port `31747` already bound on the host                                                       | Delegated to stylos's built-in port walk (cap 8, see PRD-019 §4.2). Chosen port logged at `tracing::info!` level and exposed on `/api/v1/health.stylos.listen_endpoints`.                |
| Multicast disabled on the host network (VPN, corporate LAN)                                  | Scouting silently fails; the router has no peers at startup. Operators add `[stylos].connect = ["tcp/…"]` to attach to remote peers. Documented in `docs/stele/deployment.md`.           |
| No TLS certs present for QUIC                                                                | Stylos drops the QUIC listener, keeps TCP. `/api/v1/health.stylos.listen_endpoints` reports only the `tcp/...` locator. Tray status reflects the same.                                  |
| `STELE_BIND` rebind during the session's lifetime                                            | Rebind loop restarts only the axum listener. `Arc<zenoh::Session>` is owned by `stylos_session` module above the rebind loop and continues uninterrupted. **Load-bearing** (R7 in §9).  |
| Duplicate `<instance>` across two stele-servers in the same realm                            | Two peers publish to the same key-expr. Zenoh accepts it at the transport layer; subscribers see interleaved heartbeats. Flagged as a config bug in docs; operators set distinct instances. |
| `stylos_session::open_session` fails at startup (port exhaustion, invalid config, etc.)      | Logged at `tracing::error!` level; stele-server continues without stylos (`ApiState.stylos = None`). `/api/v1/health.stylos` reports `{ enabled: true, error: "…" }`.                    |
| Process killed (SIGKILL) without running the shutdown path                                   | No `session.close()` — remote peers detect liveness loss via missing heartbeat within ~15 s (3 × period). No corruption since there's no stylos-side persistence.                        |
| Config has `[stylos]` but cargo feature `stylos` is off                                      | `[stylos]` section is parsed and ignored. A `tracing::warn!` at startup notes the mismatch so operators catch it.                                                                       |
| `[stylos].mode = "client"` with no `connect` endpoints                                       | Zenoh client mode requires an upstream router; session open fails. Error is surfaced via the same fallback path as other `open_session` failures (`ApiState.stylos = None`).             |

## 7. Migration

- **Purely additive.** No DB migration. No config migration for users who don't set `[stylos]`.
- **Default feature enablement means default mesh membership.** On upgrade from v0.16.1 → v0.17.0, every stele-server instance joins the LAN mesh with `mode = "router"` and binds port `31747` (TCP, and QUIC if certs are present).
  - Operators who need to opt out pass `--no-stylos` / set `STELE_STYLOS_ENABLED=false`, or build with `--no-default-features --features headless` (see §9 Q4 for the exact opt-out knob).
  - Operators running multiple stele-server instances on the same host must either (a) set distinct `[stylos].instance` values, and (b) rely on the stylos port walk to avoid 31747 collision.
- **Dockerfile build command changes** (load-bearing for CI): switch from `docker build apps/stele/` to `docker build -f apps/stele/Dockerfile .` (context = repo root). Document this in `docs/stele/deployment.md` and the top-level `README.md`.
- **Lock-step version bumps** via `python scripts/bump-version.py 0.17.0`. Stylos crates remain at `0.1.0` (not in the default bump set — PRD-019 §10 Q10 resolved that as a per-release decision).

## 8. Testing

No automated harness; manual smoke matches the rest of the workspace.

### 8.1 Build verification

```bash
cd apps/stele
cargo build -p stele-server                                              # desktop + stylos
cargo build -p stele-server --no-default-features --features headless    # headless + stylos
cargo clippy -p stele-server --all-features
```

### 8.2 Health endpoint smoke

Start stele-server locally, then:

```bash
# Liveness + stylos summary
curl -s http://127.0.0.1:3100/api/v1/health | jq
# Expected:
# {
#   "status": "ok",
#   "version": "0.17.0",
#   "db_ok": true,
#   "stylos": { "enabled": true, "mode": "router", "zid": "…", "peers": 0, "routers": 0, … }
# }
```

### 8.3 Heartbeat smoke (remote host)

From a second machine on the same LAN, using the stylos CLI from `apps/stylos/`:

```bash
cd apps/stylos
cargo run -p stylos-cli -- sub 'stylos/dev/stele/*/heartbeat'
# Expect a sample every 5s from each stele-server on the LAN.
```

### 8.4 Heartbeat smoke (same host, macOS multicast workaround)

Because macOS multicast loopback is unreliable (PRD-019 §4.4 / smoke-test notes), same-host tests must use explicit TCP connect:

```bash
cd apps/stylos
cargo run -p stylos-cli -- --connect tcp/127.0.0.1:31747 sub 'stylos/dev/stele/*/heartbeat'
```

### 8.5 Info queryable smoke

```bash
cd apps/stylos
cargo run -p stylos-cli -- --connect tcp/127.0.0.1:31747 get 'stylos/dev/stele/*/info'
# Expect one JSON reply per live stele-server.
```

### 8.6 Shutdown cleanliness

1. Start stele-server.
2. In another terminal, run the `sub` command from §8.3.
3. Ctrl-C stele-server (headless) or File → Quit (desktop).
4. Subscriber stops receiving heartbeats within ~5 s. No panic or hang in either process.

### 8.7 Port-walk verification

```bash
# Pre-occupy 31747
nc -l 31747 &
cargo run -p stele-server
curl -s http://127.0.0.1:3100/api/v1/health | jq '.stylos.listen_endpoints'
# Expect: ["tcp/0.0.0.0:31748"] (walked forward by 1)
```

### 8.8 Docker build

```bash
# From repo root (new context — must not cd into apps/stele/)
docker build -f apps/stele/Dockerfile -t stele-server:0.17.0 .
docker run --rm -p 3100:3100 -p 31747:31747 -p 31747:31747/udp stele-server:0.17.0
curl -s http://127.0.0.1:3100/api/v1/health | jq .stylos
```

## 9. Open Questions

1. **Config wrapping vs direct reuse of `stylos-config`.** This PRD proposes a dedicated `StyloSettings` struct in `stele-server` (§4.3) to avoid a `json5` transitive dep. The alternative — depend on `stylos-config` and deserialize it from TOML via `toml::from_str` — is simpler but drags in the stylos loader's JSON5 path. **Proposed:** dedicated struct. Confirm acceptable before landing.
2. **`<instance>` normalization ceiling.** §4.2 trims to 32 chars after grammar normalization. On hostnames longer than 32 printable-lowercase chars (rare but possible on AWS), the trim could collide. Alternative: hash the suffix. **Proposed:** 32-char trim + document the collision risk; revisit if it surfaces.
3. **`/api/v1/health` vs extending `/api/v1/stats`.** Extending `/stats` keeps the route surface small but couples liveness signal to the existing (authless, cache-unfriendly) stats payload. **Proposed:** new `/health` — idiomatic, cheaper, and lets future readiness/liveness split cleanly.
4. **Headless-no-stylos opt-out knob.** Two candidates: (a) introduce a separate `headless-minimal` feature that excludes stylos, (b) drop `stylos` from the `headless` feature array and have operators who want it compose `--no-default-features --features "headless stylos"`. Option (b) is simpler but means the default headless build does **not** join the mesh, contradicting the brief's "default on for both desktop and headless" lock. **Proposed:** (a) — keep default-on posture, provide explicit opt-out via a named feature.
5. **TLS cert story for QUIC in stele-server.** Stylos 0.1.0 drops QUIC without certs. Stele-server inherits that. For production deployments we eventually want operator-supplied certs with reload-on-SIGHUP. Out of scope for this PRD; flag as a follow-up.
6. **Cross-workspace path deps** (§4.6). The PRD commits to strategy #1 (path deps + repo-root Docker context) but the rejected alternatives (nest, publish) are worth re-examining once stylos stabilizes past 0.1.0. Revisit in the PRD that proposes publishing stylos crates.
7. **Heartbeat cadence.** 5 s is a guess. Too fast → waste on a quiet LAN; too slow → slow peer-death detection. Revisit once we have observability.
8. **Should stele-cli get a `stele stylos status` subcommand** that dumps `/api/v1/health.stylos` in human form? Out of scope for v0.17.0 but pairs naturally with this change. Tracked for a follow-up.

## 10. Implementation Checklist

Use `TaskList` / `TaskUpdate` in Claude Code to drive this. Sequence is roughly top-down; the Dockerfile and bump steps land last after the code compiles and runs.

**Pass A — Cargo feature + config scaffolding**

- [ ] Add `stylos` feature, optional `stylos-*` + `zenoh` + `hostname` deps to `apps/stele/crates/stele-server/Cargo.toml`.
- [ ] Define `StyloSettings` struct in `src/config.rs` (fields per §4.3, Serde-derived for TOML).
- [ ] Wire CLI flags and env vars in `src/main.rs` (both desktop and headless `main` entry points).
- [ ] Add `[stylos]` parsing to `src/settings.rs` (merge into the `toml::Table` preserving unknown keys).
- [ ] Build clean: `cargo build -p stele-server` with and without `--features stylos`.

**Pass B — Session module + lifecycle**

- [ ] Create `apps/stele/crates/stele-server/src/stylos_session.rs`:
  - `StylosHandle { session: Arc<zenoh::Session>, heartbeat: JoinHandle<()>, info: Queryable<…> }`.
  - `pub async fn spawn(cfg: &StyloSettings, ct: CancellationToken) -> Result<StylosHandle>` — translates `StyloSettings` to `stylos_config::StylosConfig`, calls `stylos_session::open_session`, spawns heartbeat + declares info queryable.
  - `pub async fn shutdown(self)` — aborts heartbeat, drops queryable, `session.close().await`.
  - `impl StylosHandle { pub fn session(&self) -> Arc<zenoh::Session> }` for router state.
- [ ] Implement `<instance>` derivation (§4.2) in a helper inside the module.
- [ ] Integrate into `run_server` (§4.7): open session before the rebind loop, shut it down on `ct.cancelled()`.

**Pass C — API state + health endpoint**

- [ ] Introduce `ApiState { db: DbPool, stylos: Option<Arc<zenoh::Session>> }` and migrate every `State(DbPool)` handler in `src/api.rs` (and submodules) to `State<ApiState>`.
- [ ] Add `GET /api/v1/health` handler returning the schema in §4.10.
- [ ] Document the new endpoint in `docs/stele/http-api.md`.

**Pass D — Tray (desktop)**

- [ ] Add `stylos_status_item` to the tray menu construction in `src/tray.rs`.
- [ ] Update it in `about_to_wait` by reading `StylosHandle::session().info()` (peers, routers, mode).
- [ ] Thread the session handle through `tray::run` alongside the existing `bind_state`.

**Pass E — Dockerfile + build context**

- [ ] Rewrite `apps/stele/Dockerfile` to use repo-root build context (`COPY apps/stele/`, `COPY apps/stylos/`).
- [ ] Add `/.dockerignore` at repo root (exclude `**/target/`, `**/.git/`, large doc assets).
- [ ] Update `docs/stele/deployment.md` with the new `docker build -f apps/stele/Dockerfile .` invocation.
- [ ] Verify image builds on CI (if CI exists) or locally on both x86_64 and arm64.

**Pass F — Docs**

- [ ] `docs/stele/server.md`: new section "Stylos session lifecycle" covering §4.7.
- [ ] `docs/stele/deployment.md`: `STELE_STYLOS_*` table + Dockerfile change.
- [ ] `docs/stele/http-api.md`: `/api/v1/health` reference.
- [ ] `docs/stylos/README.md`: add "Consumers" note linking to this PRD.
- [ ] `docs/README.md`: add PRD-022 row (already done in this PRD's README-update step).

**Pass G — Version bump & release**

- [ ] `python scripts/bump-version.py 0.17.0 --dry-run` and verify the affected files.
- [ ] `python scripts/bump-version.py 0.17.0`.
- [ ] Verify `apps/stele/Cargo.toml`, `apps/steop/version.go`, and both plugin `plugin.json` files all read `0.17.0`.
- [ ] Flip top-of-file `**Status:**` from `Proposed` to `Implemented (v0.17.0)` once all acceptance criteria (§8) hold.
