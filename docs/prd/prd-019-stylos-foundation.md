# PRD-019 — Stylos: zenoh-based interconnect foundation

- **Status:** Implemented (workspace v0.15.0 · stylos 0.1.0)
- **Target version:** workspace v0.15.0 · apps/stylos/ 0.1.0 (alpha)
- **Scope:** `apps/stylos/` (new), `docs/stylos/` (new), `docs/README.md`, `scripts/bump-version.py`
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Establish an interconnect foundation** for the workspace under a new top-level module `apps/stylos/`, built on [zenoh](https://zenoh.io) — a unified pub/sub, query, and storage protocol — that can eventually carry cross-process, cross-host, and cross-language signal between Stele / Steop / future tooling.
2. **Lock down the network topology & mechanism** so every later consumer inherits the same assumptions:
   - zenoh **peer mode** only at v0.1.0. Router mode and cross-network bridging are deferred.
   - Discovery: **UDP multicast on port 31746** (LAN only).
   - Data transport: **QUIC primary, TCP fallback — both on port 31747** (same port, different listeners). Locators `quic/<host>:31747` and `tcp/<host>:31747` are advertised side-by-side; peers try QUIC first and fall back to TCP on negotiation failure.
   - Port fallback: if `31747` is already bound, walk forward to `31747+N` (small capped range) and advertise whichever port was claimed.
3. **Cross-language participation.** Rust and Go **must** work as first-class peers from day one using the official native bindings. TypeScript and Python are **nice-to-have** targets and are gated on the capabilities of their upstream bindings.
4. **Ship a minimal runnable POC.** A Rust↔Go pub/sub + query/queryable exchange on a LAN (multicast discovery), driven by four tiny CLI verbs under a single `stylos` binary: `stylos pub`, `stylos sub`, `stylos get`, `stylos queryable`. The POC must exercise QUIC-then-TCP fallback at least once (one peer with QUIC disabled).
5. **Document everything under `docs/stylos/`.** README, architecture, addressing, discovery, POC, cross-language notes, plus one-line index entries in `docs/README.md`.

## 2. Non-goals

- **No integration** with `stele-server`, `steop`, or any UI in this phase. Stylos lives alongside them, not wired into them.
- **No auth, ACL, or capability model.** That is a separate PRD.
- **No payload schemas.** POC sends opaque bytes or trivial strings; no protobuf / CBOR / bincode decisions yet.
- **No WebSocket transport.** Deferred until the TS-native story is clear.
- **No TS or Python POC code.** Only a compatibility note in the docs.
- **No zenoh Storage primitive** (`zenoh-backend-*`). Pub/sub + get/queryable only.
- **No UDP unicast, Unix-domain-socket, or serial transports.** QUIC + TCP only.
- **No production packaging.** No .app bundle, no systemd unit, no Docker image for stylos at 0.1.0.
- **No tailnet / VPN / WAN transport.** LAN multicast only at v0.1.0. Cross-network bridging via explicit `connect.endpoints`, router mode, Tailscale, or any other overlay is deferred to a follow-up PRD.

## 3. Background & Motivation

### 3.1 Current state

The workspace today wires its components over HTTP/REST and MCP stdio (see [docs/architecture.md](../architecture.md)):

- `stele-server` exposes REST (`/api/v1`) and Streamable HTTP MCP.
- `stele-cli` / `stele mcp` is an HTTP client and MCP stdio proxy.
- `steop` (Go) calls `stele-server` via REST (`/api/v1/steop/*`).
- Claude Code plugins and hooks are wired through Bash invocations.

All of this is **request/response over HTTP**. There is no shared push channel, no peer-to-peer capability, and no cross-host event bus. Every new interactive feature — `st-watch`, mailbox polling, identity heartbeats — is built on top of short-poll HTTP against a single server. That scales poorly when:

- Multiple Claude Code sessions on different hosts want to see each other.
- We want reactive semantics (push events, pattern subscriptions) instead of polling.
- We want to introduce out-of-process sidecars in languages other than Rust/Go.

### 3.2 Why zenoh

[zenoh](https://zenoh.io) gives us, in one protocol and one wire format, what we'd otherwise stitch together from NATS (pub/sub), gRPC (query), and a discovery shim:

- **Pub/sub** on hierarchical key expressions (e.g. `stylos/dev/watcher/*/status`).
- **Query/queryable** — callable endpoints keyed by key-expr, with multi-responder fan-in.
- **Peer-default topology**. No mandatory broker; a router is only needed when crossing NAT or WAN.
- **Native bindings** for Rust, C, C++, Go, Python, and (in varying states) Kotlin, Java, TypeScript.
- **Runs over QUIC, TCP, UDP, TLS, and Unix sockets.** We pick QUIC + TCP.

### 3.3 Why a new top-level module

Stylos is infrastructure, not a feature of stele-server or steop. Putting it under `apps/stylos/` keeps:

- Dependency direction clean — stele/steop can later opt-in to stylos, never the other way around.
- Version cadence independent — stylos at 0.1.0 alpha while workspace rides v0.15.x.
- Language diversity explicit — stylos will host both Rust and Go code from the outset, mirroring the apps/stele (Rust) vs apps/steop (Go) split.

## 4. Design

### 4.1 Topology

- **Default mode: peer.** Every stylos process is a zenoh peer. Peers discover one another on the LAN via multicast and form a flat mesh.
- **Router mode: deferred.** Cross-network bridging (LAN ↔ LAN, tailnet, WAN) requires router mode and explicit endpoints; both are out of scope at v0.1.0 (see Non-goals).
- **No client mode** in the POC. Client mode attaches to a router and does not gossip — useful for constrained devices later, but not needed now.

```
                    LAN (multicast 224.0.0.224:31746)
  ┌──────────────────────────────────────────────────────────────┐
  │   peer_a ↔ peer_b ↔ peer_c ↔ peer_d                          │
  └──────────────────────────────────────────────────────────────┘
```

### 4.2 Ports & Transport

| Concern           | Choice                                      | Rationale                                                                   |
| ----------------- | ------------------------------------------- | --------------------------------------------------------------------------- |
| Discovery port    | `31746/udp` (multicast)                     | Override zenoh default to keep us off the same port as stock deployments    |
| Data port         | `31747` (QUIC + TCP, same port number)      | One memorable port; two listeners bound simultaneously                      |
| Primary transport | QUIC                                        | Multiplexed streams, 0-RTT, built-in TLS, better loss behavior on wifi/wan  |
| Fallback          | TCP                                         | Survives environments where UDP is blocked or QUIC TLS is mis-provisioned  |
| Port collision    | Walk forward: `31747 → 31748 → …` (cap N=8) | Let multiple stylos processes coexist on one host during dev                |
| IANA range        | 31746/31747 sit in the registered user range (1024–49151) | No known collisions; a registry sanity-check is called out in Open Qs |

Locators advertised in a peer's `listen.endpoints` (illustrative — verify against zenoh 1.x docs):

```
quic/0.0.0.0:31747
tcp/0.0.0.0:31747
```

Both listeners run simultaneously on the same port number (different sockets: UDP for QUIC, TCP for TCP). Peers attempting a connection race **QUIC first**; on handshake failure (TLS error, UDP blocked) they retry via the TCP locator.

### 4.3 Addressing & Identity

Stylos key expressions follow a fixed 4-chunk shape:

```
stylos/<realm>/<role>/<instance>
```

| Chunk      | Example              | Notes                                                                         |
| ---------- | -------------------- | ----------------------------------------------------------------------------- |
| `stylos`   | literal              | Root namespace; reserves top-level keyspace for this subsystem                |
| `<realm>`  | `dev`, `prod`, `lab` | Logical partition — think "env". Two realms never exchange messages           |
| `<role>`   | `watcher`, `cli`     | What this process is. Short lowercase identifier                              |
| `<instance>` | `host-a-42`, UUID  | Per-process identity. Host-prefix + short id keeps debug output readable      |

Illustrative key-expressions:

```
stylos/dev/watcher/host-a-42            # this peer's identity root
stylos/dev/watcher/*                    # all watchers in realm=dev
stylos/dev/*/*/status                   # status of every role, every instance
```

The zenoh key-expr grammar permits this layout; wildcard semantics (`*`, `**`) follow standard zenoh rules.

### 4.4 Discovery

> **Terminology.** Zenoh calls this mechanism **"scouting"**. Throughout this PRD and the stylos docs we use **"discovery"** in prose and headings, but the zenoh config keys are spelled `scouting.*` — they refer to the same thing. The **discovery port = scouting port = 31746/udp**.

- **LAN:** UDP multicast on `224.0.0.224:31746` (zenoh default multicast group, override port only). `scouting.multicast.enabled = true`.
- **Non-multicast networks (VPN / tailnet / WAN):** **out of scope at v0.1.0.** Zenoh supports explicit `connect.endpoints` for those cases; a follow-up PRD will specify the cross-network flow. v0.1.0 docs call out the limitation but ship no tested path.
- **Gossip:** `scouting.gossip.enabled = true` so that once two peers meet on the LAN, the rest of the mesh learns by propagation without requiring a full address book on every node.

### 4.5 Config File

Each stylos process reads a JSON5 config at startup. Default location `apps/stylos/stylos.json5` (repo-local, dev-friendly); overridable via `--config` flag and `STYLOS_CONFIG` env var.

Illustrative stub (verify against zenoh 1.x docs — exact field nesting follows upstream `zenoh::Config`):

```json5
{
  // Stylos-level identity; also used to derive the key-expr root.
  stylos: {
    realm:    "dev",
    role:     "watcher",
    instance: "host-a-42",
  },

  // Pass-through zenoh config. Keys mirror upstream zenoh.Config.
  zenoh: {
    mode: "peer",
    connect: {
      // Explicit peers / router. Empty at v0.1.0 (LAN-only); non-multicast networks deferred.
      endpoints: [],
    },
    listen: {
      endpoints: [
        "quic/0.0.0.0:31747",
        "tcp/0.0.0.0:31747",
      ],
    },
    scouting: {
      multicast: {
        enabled:   true,
        address:   "224.0.0.224:31746",
        interface: "auto",
        autoconnect: { peer: "peer", router: "peer" },
      },
      gossip: { enabled: true },
    },
    transport: {
      link: {
        tls: {
          // Dev-mode self-signed pair. Production path TBD (Open Qs §9).
          listen_private_key:  "./certs/stylos-dev.key",
          listen_certificate:  "./certs/stylos-dev.crt",
          root_ca_certificate: "./certs/stylos-dev-ca.crt",
        },
      },
    },
  },
}
```

Field names match upstream `zenoh::Config` 1.x (`listen_private_key`, `listen_certificate`, `root_ca_certificate`) — verified against the zenoh 1.9.0 default config file.

### 4.6 Crate / Module Layout

Stylos is split into **several small crates** so downstream consumers (stele, steop, future tooling) can pull in only the pieces they need without dragging in the CLI, the zenoh dependency, or the QUIC TLS plumbing. Each crate has a single, narrow responsibility.

```
apps/stylos/
├── Cargo.toml                # own [workspace] root, members = ["crates/*"]
├── crates/
│   ├── stylos-common/        # lib: shared types, errors, constants (port numbers,
│   │                         #      default multicast group, version)
│   ├── stylos-config/        # lib: JSON5 config schema, loader, validation,
│   │                         #      env-var & --config override handling
│   ├── stylos-identity/      # lib: Realm/Role/Instance types, key-expr composer
│   │                         #      (stylos/<realm>/<role>/<instance>)
│   ├── stylos-transport/     # lib: locator builders, port-walk (31747 → 31747+N),
│   │                         #      QUIC+TCP listener setup, TLS cert loading
│   ├── stylos-session/       # lib: zenoh::Session factory — consumes the four
│   │                         #      libs above, returns a ready session
│   └── stylos-cli/           # bin: `stylos` with pub/sub/get/queryable subcommands;
│                             #      depends on stylos-session only
├── go/
│   ├── go.mod                # github.com/tasanakorn/stele/apps/stylos/go
│   ├── cmd/stylos/           # Go equivalent of the same CLI (same 4 verbs)
│   └── internal/
│       ├── common/           # constants + errors (mirrors stylos-common)
│       ├── config/           # JSON5 loader (mirrors stylos-config)
│       ├── identity/         # Realm/Role/Instance (mirrors stylos-identity)
│       ├── transport/        # locator + port-walk (mirrors stylos-transport)
│       └── session/          # zenoh session factory (mirrors stylos-session)
├── stylos.example.json5      # canonical example config
├── certs/                    # dev-only self-signed pair (gitignored real keys)
└── README.md                 # points at docs/stylos/
```

Dependency direction (strict DAG — arrows mean "depends on"):

```
stylos-cli  ──► stylos-session ──► stylos-transport ──► stylos-common
                               └─► stylos-identity  ──► stylos-common
                               └─► stylos-config    ──► stylos-common
```

- **Rust side** is its **own Cargo workspace root** (does not extend `apps/stele/Cargo.toml`). Keeps `stele-common`'s dependency footprint untouched and lets stylos iterate on zenoh breaking changes without dragging stele along.
- **Reuse shape.** A future `stele-server` or `steop` consumer can `use stylos_identity::*` for key-expr composition and `use stylos_session::Session` to join the mesh — without pulling in the CLI's clap dependency or the config crate's JSON5 parser. Each crate's public surface is small enough to be audited when consumed.
- **Go side** mirrors the Rust split as `internal/` subpackages under a single `go.mod`, so the Go code can expose the same narrow primitives (`identity.Key(...)`, `session.New(...)`) when/if the same reuse pattern is applied. `internal/` keeps them out of any accidental external API until a stable split is earned.
- **Single `stylos` binary per language**, four subcommands (`pub`, `sub`, `get`, `queryable`). Revisit only if one subcommand grows disproportionately.

### 4.7 Cross-language Support

| Language   | Status at 0.1.0      | Binding                  | Notes                                                                                       |
| ---------- | -------------------- | ------------------------ | ------------------------------------------------------------------------------------------- |
| Rust       | Must (POC publisher) | `zenoh` crate            | Primary reference implementation                                                            |
| Go         | Must (POC consumer)  | `eclipse-zenoh/zenoh-go` | Peer vs client-only support is an Open Question — verify before locking POC topology       |
| TypeScript | Nice-to-have         | `zenoh-ts`               | Historically WebSocket-via-router only; current state needs verification before commitment |
| Python     | Nice-to-have         | `zenoh-python` (PyO3)    | Native, full-featured; lowest-risk addition                                                 |
| C / C++    | Not in scope         | —                        | Could follow easily; deferred                                                               |

If the Go binding turns out to be client-only in its current release, the POC adapts: the Go side attaches as a **client** to a Rust **router** peer, instead of joining the mesh as a peer. The wire-level exchange stays the same; only the Go process's `mode` differs.

### 4.8 POC Specification

**Goal:** demonstrate a Rust↔Go conversation using all four zenoh interaction primitives (pub, sub, get, queryable), in two network scenarios, with QUIC/TCP fallback observed at least once.

#### 4.8.1 CLI verbs (identical shape in Rust and Go)

| Command             | Semantics                                                                              |
| ------------------- | -------------------------------------------------------------------------------------- |
| `stylos pub <KE> <msg>` | Publish one sample to key expression `<KE>` and exit                             |
| `stylos sub <KE>`       | Subscribe to `<KE>`, print samples as they arrive, run until Ctrl-C                 |
| `stylos get <KE>`       | Send a query against `<KE>`; collect all replies for a short window; print and exit |
| `stylos queryable <KE>` | Register as a queryable on `<KE>`; serve replies until Ctrl-C                       |

All four verbs honor `--config`, `--connect <endpoint>` (repeatable, overrides config), and `--no-quic` (disables QUIC listener + connect, forcing TCP — used for the fallback test).

#### 4.8.2 Key expressions used by the POC

```
stylos/dev/poc/rust            # Rust peer publishes here, Go subscribes
stylos/dev/poc/go              # Go peer publishes here, Rust subscribes
stylos/dev/poc/echo            # Queryable lives here; both sides can `get` it
```

#### 4.8.3 Scenario — LAN multicast

Two processes, either on the same host or on two machines on the same LAN. No explicit `--connect` endpoints — discovery is pure multicast.

1. Process 1 runs `stylos queryable stylos/dev/poc/echo` (Rust).
2. Process 1 runs `stylos sub stylos/dev/poc/go` (Rust).
3. Process 2 runs `stylos sub stylos/dev/poc/rust` (Go).
4. Process 1 runs `stylos pub stylos/dev/poc/rust "hello-from-rust"`.
5. Process 2 runs `stylos pub stylos/dev/poc/go "hello-from-go"`.
6. Process 2 runs `stylos get stylos/dev/poc/echo` and receives a reply from Process 1.

**QUIC/TCP fallback variant (same LAN, one extra flag):**

1. Start Process 2 with `--no-quic`. Process 1 keeps both listeners.
2. Process 2 advertises only `tcp/...:31747`; Process 1's QUIC attempt fails, TCP succeeds, pub/sub still works.
3. Log lines (zenoh debug log or a stylos-level trace) must show the QUIC attempt and the TCP success.

#### 4.8.4 Acceptance criteria

The POC passes iff all of the following are observably true:

1. `stylos sub stylos/dev/poc/rust` on the Go peer prints `hello-from-rust` within 5s of the Rust publish.
2. `stylos sub stylos/dev/poc/go` on the Rust peer prints `hello-from-go` within 5s of the Go publish.
3. `stylos get stylos/dev/poc/echo` from either side returns at least one reply from the Rust queryable.
4. The fallback check logs a QUIC failure followed by a TCP success on the same connection attempt.
5. No stylos process requires root; both default ports (31746 UDP, 31747 UDP+TCP) are bindable in the user port range.
6. Shutting down either peer doesn't crash or hang the other.

## 5. Changes by Component

| Component                     | Change                                                                                                             | Files                                                                                              |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| apps/stylos/ (new)            | New top-level module. Own Cargo workspace, Go module, CLI binary, example config, dev certs.                        | `apps/stylos/Cargo.toml`, `apps/stylos/crates/**`, `apps/stylos/go/**`, `apps/stylos/stylos.example.json5`, `apps/stylos/README.md` |
| docs/stylos/ (new)            | New docs subtree — README, architecture, addressing, discovery, poc, cross-lang. **Shell described here; created in implementation PR, not in this PRD.** | `docs/stylos/README.md`, `docs/stylos/architecture.md`, `docs/stylos/addressing.md`, `docs/stylos/discovery.md`, `docs/stylos/poc.md`, `docs/stylos/cross-lang.md` |
| docs/README.md                | Add a "Stylos foundation (`stylos/`)" module section (placeholder pointing at this PRD) and a PRD-019 row in the PRD table. | `docs/README.md`                                                                                   |
| scripts/bump-version.py       | Add a `stylos` component entry pointing at `apps/stylos/Cargo.toml` (cargo kind). Default set unchanged; stylos follows its own cadence like `stelite`. | `scripts/bump-version.py`                                                                          |
| apps/stele/Cargo.toml         | Workspace version `0.14.0` → `0.15.0` (lock-step with plugin bumps; handled by `bump-version.py 0.15.0`).           | `apps/stele/Cargo.toml`                                                                            |
| plugins/stele/plugin.json     | Lock-step version bump to `0.15.0`.                                                                                  | `plugins/stele/.claude-plugin/plugin.json`                                                         |
| plugins/steop/plugin.json     | Lock-step version bump to `0.15.0`.                                                                                  | `plugins/steop/.claude-plugin/plugin.json`                                                         |
| apps/steop/version.go         | Lock-step version bump to `0.15.0`.                                                                                  | `apps/steop/version.go`                                                                            |

No changes to `apps/stele/crates/**`, `apps/steop/cmd_*.go`, or any plugin skill. Stylos is purely additive.

## 6. Edge Cases

| Scenario                                                                                  | Behavior                                                                                                                                               |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Port `31747` already bound on the host                                                    | Walk forward to `31747+N` (cap N=8). Advertise the chosen port in the peer's listen endpoints. Log the chosen port. Fail loudly if all slots are taken. |
| UDP multicast blocked (corporate LAN, some VPNs)                                          | Scouting silently fails; peers never discover each other via multicast. User must supply `connect.endpoints`. Documented in `docs/stylos/discovery.md`. |
| QUIC TLS cert missing or expired                                                          | QUIC listener fails to bind. Stylos logs a warning, continues with TCP listener only. Remote peers see only the `tcp/...` locator.                      |
| One peer has `--no-quic`, the other has QUIC required                                     | Negotiation falls back to TCP automatically — both peers advertise TCP locators too, so no manual intervention.                                        |
| Two stylos processes on one host with the same `(realm, role, instance)`                  | zenoh allows it at the transport layer, but the duplicated key-expr root causes observable duplication. Treated as a config bug; documented.            |
| Go binding is client-only in the released version                                         | POC Go process runs in `mode: client` and attaches to a Rust router. All application-level verbs still work. Flagged as Open Q §9.                      |
| Zenoh version skew between Rust and Go peers                                              | Zenoh wire protocol is stable across recent minor versions; POC pins specific versions in `Cargo.toml` and `go.mod` to remove ambiguity.                |
| `--connect` given but endpoint unreachable                                                | Zenoh retries in the background; stylos reports connection failures via its log but does not exit. Matches zenoh default behavior.                      |

## 7. Migration

- **Purely additive.** No existing file's behavior changes. No DB migration. No stele/steop code touched.
- **Version bump is lock-step** for the workspace + stele/steop plugins (`0.14.0 → 0.15.0`) via `scripts/bump-version.py 0.15.0`. `stylos` starts at `0.1.0` alpha and is **not** part of the default component set — it bumps on its own cadence (same rule as `stelite`).
- **Nothing to uninstall or replace.** If a user never runs `stylos`, nothing changes for them.

## 8. Testing

No automated test harness exists yet in the workspace; stylos follows the same manual-smoke posture as the rest of the repo until a fixture story emerges.

1. **Build both sides:** `cd apps/stylos && cargo build` (Rust CLI) and `cd apps/stylos/go && go build -o target/stylos ./cmd/stylos` (Go CLI). Both must produce a `stylos` binary.
2. **Unit sanity:** `cargo test -p stylos-core` covers key-expr formatting and config parsing. Go side mirrors with `go test ./internal/stylos/...`.
3. **Scenario (LAN):** run the 6-step exchange in §4.8.3. All acceptance criteria §4.8.4.1–6 must hold.
4. **QUIC/TCP fallback:** launch the Go peer with `--no-quic` on the same LAN. Verify log lines show QUIC attempt + TCP success.
5. **Port walk:** pre-occupy `31747` with `nc -l 31747`, start stylos, confirm it lands on `31748` and advertises it.
6. **Version bump dry-run:** `python scripts/bump-version.py 0.15.0 --dry-run` lists workspace + stele + steop moving to 0.15.0 and stylos unchanged; `... 0.1.1 stylos --dry-run` lists only stylos moving.

## 9. Open Questions

1. **Zenoh-go peer mode status.** Does `eclipse-zenoh/zenoh-go` support full peer mode in its current release, or is it still client-only? This decides whether the Go POC joins the mesh directly or attaches to a Rust router. **Verify via upstream docs / GitHub README before implementation.**
2. **Zenoh-ts transport story.** Is `zenoh-ts` still WebSocket-via-router-plugin only, or has it gained native QUIC/TCP? This gates whether TS is a realistic "native" addition or must route through a Rust bridge. **Verify before committing to TS scope.**
3. **Default scouting port (7446).** Brief states zenoh default is `224.0.0.224:7446`. Verify exact default for the pinned zenoh version before landing docs.
4. **`transport_quic` feature flag.** Brief notes QUIC may require the `transport_quic` feature on the `zenoh` crate in some versions (default-on in recent releases). Verify for the version we pin and document if `--features transport_quic` is needed.
5. **QUIC cert lifecycle.** Three candidate models: (a) ship a dev cert in the repo, (b) generate one on first run and stash under `~/.local/share/stylos/`, (c) operator-supplied path only. Recommend (a) for the POC, (b) for 0.2.0, (c) as the only prod option. Not locked in this PRD. **Resolved for 0.1.0:** shipped as option (a) via `apps/stylos/scripts/gen-dev-certs.sh`. 0.2.0 posture deferred.
6. **Session model.** One `zenoh::Session` per process, or per role? Current proposal is one-per-process; per-role adds isolation at the cost of multiple listeners. **Resolved:** one `zenoh::Session` per process (simpler; no observed need for per-role isolation).
7. **Cargo workspace membership.** Should `apps/stylos/` extend `apps/stele/Cargo.toml` as a second root, or be wholly independent? Current proposal is fully independent; revisit if duplicated dep trees become painful. **Resolved:** fully independent `apps/stylos/Cargo.toml` workspace root.
8. **CLI packaging.** Single `stylos` binary with subcommands (current proposal) vs per-verb binaries (`stylos-pub`, `stylos-sub`, …). Single binary wins on DX; revisit only if one subcommand grows disproportionately. **Resolved:** single `stylos` binary with 5 subcommands (`pub`/`sub`/`get`/`queryable`/`identity`).
9. **Port-fallback policy.** Cap and advertise: the POC caps the walk at N=8 and logs the chosen port to stdout. Whether to expose this via a sidecar file / RPC for peers on the same host is deferred. **Resolved:** cap = 8 forward-walk; chosen port logged to stdout at session open.
10. **`bump-version.py` integration timing.** Land the `stylos` component entry in the same PR as the v0.15.0 bump, or defer to the first actual stylos release? Current proposal: land together, since the entry is passive until stylos is explicitly named on the CLI. **Resolved:** `stylos` component entry landed in Pass A alongside the v0.15.0 bump.
11. **Port registry sanity check.** Ports 31746/31747 appear unclaimed in the IANA registered range, but a quick registry lookup before v0.1.0 ships is worth doing. **Resolved:** 31746/31747 confirmed unclaimed in IANA registry (checked 2026-04-14).

## 10. Implementation Checklist

Snapshot of remaining work as of 2026-04-14, after Pass A (scaffolding) and Pass B-Rust (zenoh integration + smoke test) shipped. Cross-referenced with the session task list; use `TaskList` / `TaskUpdate` in Claude Code to drive the work.

### Done

- [x] **Pass A — Scaffolding (v0.15.0).** `apps/stylos/` workspace + 6 crate shells, Go submodule stubs, `stylos.example.json5`, `docs/stylos/` shells, `docs/README.md` module entry, `bump-version.py` registered stylos, lock-step 0.14.0 → 0.15.0 bump.
- [x] **Pass B-0 — Research.** Verified zenoh 1.x Rust + zenoh-go 1.9.0 APIs from primary sources; findings durably captured in [research-b0.md](../stylos/research-b0.md). Resolved §9 Q1 (zenoh-go peer mode supported), Q3 (default scouting port 7446), Q4 (`transport_quic` default-on).
- [x] **Pass B-Rust — Zenoh integration.** All 5 Rust libs implemented (common, config, identity, transport, session) + `stylos` CLI with 5 subcommands (pub/sub/get/queryable/identity). zenoh pinned `=1.9.0`; config built entirely via stable `insert_json5`. Port walk (TCP+UDP dual bind), auto-fallback to TCP when no TLS certs. Dev cert generator (`apps/stylos/scripts/gen-dev-certs.sh`). Smoke test script (`apps/stylos/scripts/smoke-test.sh`) passing §4.8.4 criteria 1, 3, 5, 6. `docs/stylos/{architecture,addressing,discovery}.md` fleshed out. `apps/stylos/README.md` quickstart.
- [x] **§4.8.4 full coverage.** Criteria 1–6 all validated on single-host loopback by four smoke scripts: `smoke-test.sh` (1, 3, 5, 6), `quic-fallback-test.sh` (4), `go-interop-test.sh` (3 Rust→Go direction), `go-pub-rust-sub-test.sh` (2). Two-host LAN field validation deferred to 0.2.0+.

### Remaining

**Pass B-Go — Go peer implementation**

- [x] Populate `apps/stylos/go/` (mirror of Rust split via `internal/` subpackages)
- [x] `stylos` Go binary with same 4 verbs (pub/sub/get/queryable)
- [x] Resolve CGO blocker: zenoh-go v1.9.0 requires `zenoh-c` installed with `-DZENOHC_BUILD_WITH_UNSTABLE_API=ON`. Decide: document prereq, or vendor an install script.
- [x] Go ↔ Rust LAN interop test (§4.8.4 criterion 2 — Rust sub receives Go pub within 5s)

**Pass B-Docs — Remaining doc shells**

- [ ] `docs/stylos/poc.md` — POC scenario (§4.8.3), acceptance criteria (§4.8.4), smoke-test run instructions, QUIC/TCP fallback procedure, two-host LAN procedure
- [ ] `docs/stylos/cross-lang.md` — Rust/Go/TS/Python binding status matrix (§4.7) + zenoh-c prereq for Go
- [ ] `docs/stylos/README.md` — subtree index (currently still TBD stub)

**QUIC fallback observability (§4.8.4 criterion 4)**

- [x] Run `gen-dev-certs.sh`, start one peer with QUIC enabled + another with `--no-quic`, confirm `RUST_LOG=zenoh=debug` shows QUIC attempt + TCP success
- [x] Add `--no-quic` variant (or separate script) to `smoke-test.sh` asserting both listeners advertised and fallback logged

**Deferred to 0.2.0+ field testing (two-host LAN scenario, §4.8.3)**

- [ ] Run on two separate machines on the same LAN with pure multicast discovery (no explicit `--connect`) to validate scouting works when the macOS multicast-loopback caveat doesn't apply
- [ ] Record outcome in `docs/stylos/poc.md`

**PRD hygiene**

- [x] §4.5 example: rename `server_private_key` / `server_certificate` → `listen_private_key` / `listen_certificate` (upstream zenoh names). Implementation is already correct; the PRD example is stale.
- [x] §9 Q5 (QUIC cert lifecycle): 0.1.0 went with option (a) via `gen-dev-certs.sh`. Lock 0.2.0 posture (recommend option (b) — generate on first run under `~/.local/share/stylos/`).
- [x] §9 Q6 (session model): resolved → one-per-process.
- [x] §9 Q7 (cargo workspace membership): resolved → fully independent.
- [x] §9 Q8 (CLI packaging): resolved → single `stylos` binary with subcommands.
- [x] §9 Q9 (port-fallback policy): resolved → cap 8, logs chosen port to stdout; sidecar-file exposure deferred.
- [x] §9 Q10 (`bump-version.py` integration timing): resolved → landed together in Pass A.
- [x] §9 Q11 (IANA port registry sanity check): do the lookup; update status.
- [x] Update the top-of-file `**Status:**` line when Pass B-Go lands and all §4.8.4 criteria pass — flip from "Partially implemented" to "Implemented (workspace v0.15.0 · stylos 0.1.0)".

**Optional / deferred (separate PRDs)**

- [ ] TS / Python "nice-to-have" bindings (§4.7) — blocked on zenoh-ts transport story (§9 Q2)
- [ ] Router-mode / cross-LAN bridging (§2 non-goal)
- [ ] Automated test harness (repo has no test infrastructure yet)
