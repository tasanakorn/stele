# PRD-023 — Drop QUIC from stylos, default to UDP

- **Status:** Implemented (v0.18.0)
- **Target version:** workspace v0.18.0 (stylos crates stay at `0.1.0`; this PRD trims but does not re-version them)
- **Scope:** `apps/stele/crates/stele-server/src/{stylos_module,settings,config}.rs`, `apps/stylos/crates/stylos-config/src/lib.rs`, `apps/stylos/crates/stylos-session/src/lib.rs`, `apps/stylos/crates/stylos-transport/src/lib.rs`, `apps/stylos/crates/stylos-cli/src/main.rs`, `apps/stylos/stylos.example.json5`, `apps/stylos/scripts/{quic-fallback-test.sh,gen-dev-certs.sh,smoke-test.sh,go-interop-test.sh,go-pub-rust-sub-test.sh}`, docs (`docs/stele/deployment.md`, `docs/stele/server.md`, `docs/stele/http-api.md`, `docs/stylos/discovery.md`, `docs/stylos/poc.md`, `apps/stylos/README.md`, `docs/README.md`), PRD cross-refs (superseded-by notes in PRD-019 §4.2/§9 and PRD-022 §4.4/§4.9/§6), lock-step version bump (`apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`).
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

---

## 1. Goals

1. **Replace QUIC listen endpoints with UDP** as the default data-plane transport on port `31747`. Post-PRD, a fresh stylos session advertises `["udp/0.0.0.0:31747", "tcp/0.0.0.0:31747"]` — same port number, two protocol listeners, no TLS handshake gating.
2. **Remove QUIC support end-to-end** from `stylos-config`, `stylos-transport`, `stylos-session`, `stylos-cli`, and the stele-server embedding. After this PRD, no crate in the workspace references `quic/`, TLS certificates, or the `no_quic` override. The TLS config section (`root_ca_certificate`, `listen_private_key`, `listen_certificate`) disappears from stylos entirely.
3. **Retire the `--stylos-no-quic` flag and `STELE_STYLOS_NO_QUIC` env var** from stele-server. This is a breaking CLI-surface change — users who have either in their shell rc, systemd unit, or Docker `-e` list will see an unrecognised-argument error on upgrade. The breakage justifies the minor bump (0.17 → 0.18).
4. **Delete all QUIC-flavored dev infrastructure:** the `quic-fallback-test.sh` and `gen-dev-certs.sh` scripts, the `[stylos].transport.link.tls` block in the example config, and every `--no-quic` invocation in `smoke-test.sh`, `go-interop-test.sh`, and `go-pub-rust-sub-test.sh`.
5. **Rewrite the default-transport narrative in docs** so that every doc referencing stylos transports says "UDP + TCP" rather than "QUIC + TCP". Historical PRDs (PRD-019, PRD-022) are preserved verbatim with added "Superseded-by PRD-023" notes rather than rewritten.

## 2. Non-goals

- **No TCP transport changes.** TCP stays at `tcp/0.0.0.0:31747`, still dual-bound on the same port number via the existing `walk_available_port` probe.
- **No new TLS story.** Stylos shipped without a cert path from PRD-019 onward; this PRD does not introduce DTLS, UDP-over-TLS, or any operator-supplied cert reload. Hardening is a future PRD concern, out of scope here.
- **No zenoh-pico client work.** Unblocking a future zenoh-pico peer is the motivation for this PRD, not a deliverable. No C/C++ binding, no embedded example, no pico pub/sub demo lands in v0.18.0.
- **No RPC schema change.** The `stylos/<realm>/stele/<instance>/info` queryable still returns the PRD-022 §4.9 schema; only the example value for `listen_endpoints` flips from `quic/…` to `udp/…`. The heartbeat payload (`b"alive"`) is unchanged.
- **No stylos crate version bump.** Stylos crates stay at `0.1.0` per PRD-019 §10 Q10 and PRD-022 §7 precedent — they are path-deps, not published, and the workspace minor bump is enough to signal the breaking change.
- **No automated test harness.** Manual smoke posture matches the rest of the workspace; the deleted `quic-fallback-test.sh` is not replaced by an equivalent UDP-fallback script.

## 3. Background & Motivation

### 3.1 Current state

[PRD-019](prd-019-stylos-foundation.md) landed `apps/stylos/` in v0.15.0 with a TCP+QUIC dual-listener design on port `31747` (§4.2 L86–93: "QUIC primary, TCP fallback — both on port 31747"). The rationale (PRD-019 L90) was multiplexed streams, 0-RTT reconnect, and built-in TLS. The implementation landed three moving parts:

- **Config schema:** `ZenohSection.transport: Option<TransportSection>` with a nested `LinkSection.tls: Option<TlsSection>` carrying `root_ca_certificate`, `listen_private_key`, and `listen_certificate` fields (`apps/stylos/crates/stylos-config/src/lib.rs:54–55,117–138`). Upstream zenoh 1.9 names verbatim.
- **Transport helper:** `listen_endpoints(port, quic_enabled)` returns `["quic/0.0.0.0:<p>", "tcp/0.0.0.0:<p>"]` when `quic_enabled`, else TCP-only. Separate `TlsPaths::from_config` translates the config struct into filesystem paths for zenoh's `insert_json5` writes (`apps/stylos/crates/stylos-transport/src/lib.rs:8–14,27–48`).
- **Session factory gate:** `stylos_session::open_session` computes `let quic_allowed = !overrides.no_quic && tls_configured;` and filters `quic/` endpoints out of the listen list when `quic_allowed` is false (`apps/stylos/crates/stylos-session/src/lib.rs:44–60,93–107`). Silently drops QUIC with a warning when no TLS certs are present.

[PRD-022](prd-022-stylos-in-stele-server.md) embedded this into stele-server in v0.17.0 and propagated the override surface:

- `StyloSettings.no_quic: bool` (`apps/stele/crates/stele-server/src/settings.rs:48`) with `serde(default)` to `false`.
- `Config.stylos_no_quic` CLI flag + `STELE_STYLOS_NO_QUIC` env (`apps/stele/crates/stele-server/src/config.rs:59–61`) and the merge path that sets `base.no_quic = true` when the flag is present (L103–105).
- `SessionOverrides { no_quic }` is constructed from the setting at `apps/stele/crates/stele-server/src/stylos_module.rs:156–163` and handed to `open_session` at L165.
- The example health payload in `docs/stele/http-api.md:50` shows `["tcp/0.0.0.0:31747","quic/0.0.0.0:31747"]`; the live endpoint returns `Vec::new()` because zenoh 1.9 exposes no stable listener enum (documented at PRD-022 §4.9 L253 and `apps/stele/crates/stele-server/src/stylos_module.rs:42`).

### 3.2 Why QUIC has been dormant since PRD-019

PRD-019 landed the QUIC code path but shipped no cert story. The TLS block in `stylos.example.json5` (L26–34) points at a `./certs/` directory that only exists if the operator ran `gen-dev-certs.sh` by hand. Every dev script in `apps/stylos/scripts/` (§3.1 deltas above, plus `smoke-test.sh:10,77,86,94,118`, `go-interop-test.sh:79`, `go-pub-rust-sub-test.sh:81`) passes `--no-quic` to force the TCP fallback because running without certs trips the `!tls_configured` branch in `open_session` (L47). The one script that exercises QUIC — `quic-fallback-test.sh` — is a single-point verification for the fallback behavior itself, not a normal-operation test.

In stele-server (PRD-022 §4.3 L118, §4.4 L141), `no_quic = false` is the default but has no observable effect on a fresh install because stele-server likewise ships no certs. The `StyloSettings.no_quic` field is in practice a `false` that triggers a `!tls_configured` warning at startup rather than an actual QUIC listener.

### 3.3 Why UDP default now

- **zenoh-pico, the target lightweight peer for future embedded/edge work, does not support QUIC.** A stele-server that advertises only QUIC + TCP cannot be joined by a pico peer on the QUIC path; forcing pico to TCP-only is fine but carrying a QUIC code path that nothing uses is drag. UDP is the zenoh-pico-friendly datagram transport and is what the pico examples target.
- **Dormant features rot.** Every config knob, CLI flag, env var, and script line that exists for a disabled feature is a future maintenance hazard — each one is a point where someone can mis-read the system's actual behavior. PRD-022's Open Questions §9 Q5 explicitly deferred the TLS cert story; this PRD cashes that deferral as removal rather than implementation.
- **The breaking-flag cost is low.** `STELE_STYLOS_NO_QUIC` and `--stylos-no-quic` are ~6 months old and almost certainly not set anywhere in production — they were introduced in PRD-022 (v0.17.0) with `no_quic = false` as the default, which is also what happens when the flag is absent. Removing them cannot change behavior for anyone who was using the default.
- **Single shared port number.** Port `31747` stays as the data-plane reservation (`apps/stylos/crates/stylos-common/src/lib.rs:6`). UDP and TCP are distinct sockets at the OS level, and `walk_available_port` already dual-probes TCP + UDP for availability (`apps/stylos/crates/stylos-transport/src/lib.rs:17–20`) — the current code was already doing a UDP-probe on every port walk for a listener it never advertised. This PRD brings the advertised transport set into sync with what the probe already enforces.

## 4. Design

### 4.1 Transport model

After this PRD, a stylos session opened with default settings advertises two listen endpoints on the same port:

```
udp/0.0.0.0:31747
tcp/0.0.0.0:31747
```

Both listeners are unconditional when the port is free. `walk_available_port` keeps its existing dual-TCP-plus-UDP probe semantics — it already required both sockets to bind successfully before accepting a port (L17–24). The transport helper `listen_endpoints(port)` loses its `quic_enabled: bool` parameter and always returns the two-entry vector above.

**Alternative considered: UDP-only listener.** Going UDP-only would simplify the listen list to one entry and free TCP from the port reservation. Rejected because (a) `walk_available_port` probes both protocols and a UDP-only story would need either a second function or a policy change, and (b) TCP remains the most interoperable fallback for zenoh peers on constrained networks (VPN, corporate firewalls that drop UDP). Keeping TCP is free and preserves the PRD-019 multi-transport posture minus the QUIC layer.

### 4.2 Removed config surface

The entire TLS configuration path in `stylos-config` is deleted:

| Deleted struct       | File                                               | Line range |
| -------------------- | -------------------------------------------------- | ---------- |
| `TransportSection`   | `apps/stylos/crates/stylos-config/src/lib.rs`     | 117–121    |
| `LinkSection`        | `apps/stylos/crates/stylos-config/src/lib.rs`     | 123–127    |
| `TlsSection`         | `apps/stylos/crates/stylos-config/src/lib.rs`     | 129–138    |

The `ZenohSection.transport` field (L54–55) and its `Default` impl assignment (L65) are deleted in lock-step — the struct disappears, the field referring to it disappears, and `stylos-session` loses all `TlsPaths::from_config` callers.

User-facing consequence: a `stylos.json5` config file that still contains a `transport: { link: { tls: {...} } }` block will **fail to parse** — Serde rejects unknown fields unless the containing struct opts into `#[serde(deny_unknown_fields)]` or `#[serde(flatten)]`, neither of which `ZenohSection` uses. The top-level `StylosConfig` has no `#[serde(default)]` escape for unknown fields, so legacy configs must be edited on upgrade. Migration (§7) documents the one-line fix.

### 4.3 Removed override surface

`SessionOverrides.no_quic` (`apps/stylos/crates/stylos-session/src/lib.rs:11`) is deleted. The struct shrinks to:

```rust
#[derive(Debug, Clone, Default)]
pub struct SessionOverrides {
    pub connect: Option<Vec<String>>,
}
```

Every call site adjusts. In `stylos-cli`, `CommonArgs.no_quic: bool` (`apps/stylos/crates/stylos-cli/src/main.rs:24`) and the matching arm in `overrides_from` (L77) are deleted. In `stele-server`, `StyloSettings.no_quic` and its default function are deleted from `settings.rs:48,59`, `Config.stylos_no_quic` is deleted from `config.rs:59–61`, the merge branch is deleted from `config.rs:103–105`, and the `no_quic: settings.no_quic` field in the `SessionOverrides` constructor is deleted from `stylos_module.rs:162`.

`open_session` loses the `tls_configured` probe (`apps/stylos/crates/stylos-session/src/lib.rs:44–48`), the `quic_allowed` gate (L45), the QUIC-filtering branch in listen-endpoint selection (L54–57), and the entire "Apply TLS paths to zenoh config" block (L93–107). The body simplifies to: compute `listen_endpoints(port)` unconditionally, set connect endpoints from overrides-or-config, wire scouting, open session.

### 4.4 Removed CLI/env surface

Two stele-server flags and two env vars disappear:

| Removed flag             | Removed env var            | Was mapped to                    |
| ------------------------ | -------------------------- | -------------------------------- |
| `--stylos-no-quic`      | `STELE_STYLOS_NO_QUIC`    | `StyloSettings.no_quic`          |

One stylos-cli flag disappears:

| Removed flag   | Was mapped to                     |
| -------------- | --------------------------------- |
| `--no-quic`   | `SessionOverrides.no_quic`        |

The PRD-022 §4.4 flag table is reduced by one row; every other stylos override flag (`--stylos`, `--no-stylos`, `--stylos-mode`, `--stylos-realm`, `--stylos-instance`, `--stylos-connect`) survives untouched.

### 4.5 Removed dev-script surface

| Script                                              | Change                                                                                       |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `apps/stylos/scripts/quic-fallback-test.sh`        | Delete file. Covered the PRD-019 §4.8.4 criterion-4 fallback that no longer exists.          |
| `apps/stylos/scripts/gen-dev-certs.sh`             | Delete file. Only consumer was `quic-fallback-test.sh` and the example-config `certs/` path. |
| `apps/stylos/scripts/smoke-test.sh`                | Drop the 5 `--no-quic` occurrences (L77, L86, L94, L118) and the L10 header comment.         |
| `apps/stylos/scripts/go-interop-test.sh`           | Drop the single `--no-quic` (L79).                                                           |
| `apps/stylos/scripts/go-pub-rust-sub-test.sh`      | Drop the single `--no-quic` (L81).                                                           |

### 4.6 Go mirror

A scan of `apps/stylos/go/{cmd,internal}/*` at PRD-authoring time found **zero** `quic|QUIC|NoQUIC|NoQuic|no_quic|no-quic` occurrences. The Go binding was ported after the QUIC-gate logic stabilised and tracked upstream zenoh-go defaults, which in this workspace never exercised QUIC. No Go source changes are expected; the implementer should rerun the same grep before declaring done and list any stragglers inline.

### 4.7 Example-config rewrite

`apps/stylos/stylos.example.json5` loses two chunks:

1. The L14 hint comment (`// Or pin: ["quic/0.0.0.0:31747", "tcp/0.0.0.0:31747"]`) flips to `// Or pin: ["udp/0.0.0.0:31747", "tcp/0.0.0.0:31747"]`.
2. The L26–34 `transport: { link: { tls: { ... } } }` block is deleted outright.

After the change, the example config matches the post-PRD `StylosConfig` schema exactly and will round-trip through `StylosConfig::load` without warnings.

### 4.8 Docs rewrite vs superseded-by note

Historical PRDs are **not** rewritten. PRD-019 and PRD-022 are decision records; editing their bodies after the fact destroys the archaeology of why the current state exists. Instead:

- Add a one-paragraph admonition at the top of PRD-019 §4.2 ("QUIC primary, TCP fallback") pointing at PRD-023 as the superseding decision. Same treatment for PRD-019 §9 (the TLS cert story) and PRD-022 §4.4 flag table, §4.9 queryable example, and §6 QUIC-drop edge case.
- **Do** rewrite the docs that describe *current* behavior: `docs/stele/deployment.md` (env var table), `docs/stele/server.md` (the "TLS / QUIC cert hardening deferred" line), `docs/stele/http-api.md` (health-endpoint example body), `docs/stylos/discovery.md` (data-listeners section), `docs/stylos/poc.md` (QUIC fallback scenario), `apps/stylos/README.md` (test scripts + `--no-quic` flag entry), and the one-line PRD row in `docs/README.md`.
- `docs/stylos/research-b0.md` L12,16,44,159–205 cites `transport_quic` as upstream zenoh background. Leave intact — it is a research note, not a current-behavior spec.

### 4.9 Health-endpoint example body

The PRD-022 §4.9 `info` queryable and PRD-022 §4.10 `/api/v1/health` endpoint schemas are unchanged; only the **example** `listen_endpoints` literal in the docs flips from `["tcp/0.0.0.0:31747", "quic/0.0.0.0:31747"]` to `["udp/0.0.0.0:31747", "tcp/0.0.0.0:31747"]`. The runtime value remains `Vec::new()` because zenoh 1.9 still exposes no stable listener enum (PRD-022 §4.9 L253 rationale unchanged).

### 4.10 Transitive QUIC deps

`apps/stele/Cargo.lock` and `apps/stylos/Cargo.lock` carry `quinn`, `quinn-proto`, `quinn-udp`, `rustls`, `rustls-pemfile`, `rustls-pki-types`, `rustls-webpki`, `tokio-rustls`, and `webpki-roots` as transitive deps of `zenoh = "=1.9.0"`. zenoh 1.9 exposes **no** feature flag to disable its QUIC transport layer at compile time (confirmed in the PRD-019 §9 Q4 research and not altered by any 1.9.x patch since). This PRD accepts the transitive closure for v0.18.0 and defers the cleanup to either (a) a zenoh 2.x upgrade that feature-gates QUIC, or (b) an upstream PR to add a `-zenoh-transport-quic` feature. Neither is a v0.18.0 blocker.

### 4.11 Version bump

Lock-step bump from 0.17.x → 0.18.0 via `python scripts/bump-version.py 0.18.0`. Five files move:

- `apps/stele/Cargo.toml` (workspace version).
- `apps/stele/Cargo.lock` (auto-regenerates; re-commit the diff).
- `apps/steop/version.go`.
- `plugins/stele/.claude-plugin/plugin.json`.
- `plugins/steop/.claude-plugin/plugin.json`.

Stylos crates (`apps/stylos/crates/*/Cargo.toml`) stay at `0.1.0` per the PRD-022 §7 precedent — they are path-deps only, their API changes shape in this PRD, but there is no published consumer to bump against.

## 5. Changes by Component

| Component                                | File(s)                                                                                             | Change                                                                                                                                                                                                                              |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| stylos-config schema                    | `apps/stylos/crates/stylos-config/src/lib.rs:54–55,65,117–138`                                     | Delete `TransportSection`, `LinkSection`, `TlsSection`. Delete `ZenohSection.transport` field + its `Default` assignment. No new fields added.                                                                                       |
| stylos-transport helper                 | `apps/stylos/crates/stylos-transport/src/lib.rs:8–14,27–48`                                        | Change `listen_endpoints(port: u16, quic_enabled: bool) -> Vec<String>` to `listen_endpoints(port: u16) -> Vec<String>`; body returns `["udp/0.0.0.0:{port}", "tcp/0.0.0.0:{port}"]`. Delete `TlsPaths` struct and `from_config`.     |
| stylos-session factory                  | `apps/stylos/crates/stylos-session/src/lib.rs:3,5,8–12,42–60,93–107`                               | Delete `SessionOverrides.no_quic`. Delete `TlsPaths` import. Delete the `tls_configured`/`quic_allowed` gate. Delete the TLS `insert_json5` block. `listen_endpoints(port)` call loses second arg.                                  |
| stylos-cli flag                         | `apps/stylos/crates/stylos-cli/src/main.rs:7,18–25,74–79`                                          | Delete `CommonArgs.no_quic` field. Delete `no_quic` from `overrides_from`. Import of `TlsPaths` via `stylos_session` already doesn't exist; no further import changes.                                                               |
| stele-server CLI surface                | `apps/stele/crates/stele-server/src/config.rs:59–61,103–105`                                       | Delete `Config.stylos_no_quic` arg + env var. Delete the merge branch that sets `base.no_quic = true`.                                                                                                                              |
| stele-server persisted settings         | `apps/stele/crates/stele-server/src/settings.rs:47–48,59`                                          | Delete `StyloSettings.no_quic` field. Delete its `Default` impl line.                                                                                                                                                                |
| stele-server session adapter            | `apps/stele/crates/stele-server/src/stylos_module.rs:156–163`                                      | Drop `no_quic: settings.no_quic` from the `SessionOverrides` literal. No other logic change in this file; health response body already returns `Vec::new()` for `listen_endpoints`.                                                  |
| Stylos example config                   | `apps/stylos/stylos.example.json5:14,26–34`                                                        | Flip the L14 pin-hint comment to `udp/…` + `tcp/…`. Delete the L26–34 `transport.link.tls` block.                                                                                                                                    |
| Dev script: delete                      | `apps/stylos/scripts/quic-fallback-test.sh`                                                         | Delete file.                                                                                                                                                                                                                         |
| Dev script: delete                      | `apps/stylos/scripts/gen-dev-certs.sh`                                                              | Delete file.                                                                                                                                                                                                                         |
| Dev script: flag drop                   | `apps/stylos/scripts/smoke-test.sh:10,77,86,94,118`                                                 | Delete the L10 header comment and 4 `--no-quic` arguments. Body logic unchanged.                                                                                                                                                     |
| Dev script: flag drop                   | `apps/stylos/scripts/go-interop-test.sh:79`                                                         | Delete the `--no-quic` argument.                                                                                                                                                                                                     |
| Dev script: flag drop                   | `apps/stylos/scripts/go-pub-rust-sub-test.sh:81`                                                    | Delete the `--no-quic` argument.                                                                                                                                                                                                     |
| Go mirror verification                  | `apps/stylos/go/**`                                                                                 | Re-run `grep -rE 'quic\|QUIC\|NoQUIC\|NoQuic\|no_quic\|no-quic'` before declaring done. PRD-authoring grep returned zero; implementer confirms at execute-time and lists any findings inline.                                       |
| Docs — workspace index                  | `docs/README.md:83`                                                                                 | Rewrite "QUIC+TCP transport" prose in the `stylos/architecture.md` row to "UDP+TCP transport". Add PRD-023 row after the PRD-022 row, ordered numerically.                                                                           |
| Docs — deployment                       | `docs/stele/deployment.md:135`                                                                      | Delete the `STELE_STYLOS_NO_QUIC` env-var row from the stele-server env-var table. No replacement row.                                                                                                                               |
| Docs — server internals                 | `docs/stele/server.md:87`                                                                           | Delete the "TLS / QUIC cert hardening deferred" bullet.                                                                                                                                                                              |
| Docs — HTTP API                         | `docs/stele/http-api.md:50,57`                                                                      | Flip the `/api/v1/health.stylos.listen_endpoints` example value from `["tcp/…","quic/…"]` to `["udp/…","tcp/…"]`. Update the "reported as empty" note to mention UDP + TCP instead of TCP + QUIC.                                    |
| Docs — stylos discovery                 | `docs/stylos/discovery.md:15–28,41`                                                                 | Rewrite the "Data listeners" section end-to-end: replace QUIC description with UDP description (datagram on `31747`, no TLS, paired with TCP). Update the L41 example locator list.                                                  |
| Docs — stylos POC                       | `docs/stylos/poc.md:11,30,41,48–69`                                                                 | Delete the QUIC fallback scenario (L48–69) and the QUIC mentions at L11, L30, L41. No replacement scenario — UDP is the default, not a fallback.                                                                                     |
| Docs — stylos README                    | `apps/stylos/README.md:52,80–84,97`                                                                 | Delete `quic-fallback-test.sh` from the scripts table. Delete the "QUIC/TCP fallback test" section. Delete the `--no-quic` flag row.                                                                                                 |
| PRD cross-ref (historical, preserve)    | `docs/prd/prd-019-stylos-foundation.md` §4.2, §9                                                    | Add "**Superseded by PRD-023 (v0.18.0).** Stylos now defaults to UDP + TCP; QUIC and the TLS cert path are removed." admonition at top of §4.2 and §9. Do not edit the body prose.                                                   |
| PRD cross-ref (historical, preserve)    | `docs/prd/prd-022-stylos-in-stele-server.md` §4.4 (flag table), §4.9 (queryable example), §6 (QUIC edge case) | Add inline footnotes / admonitions pointing at PRD-023. Do not edit the tables or prose.                                                                                                                                  |
| Version bump                            | `apps/stele/Cargo.toml`, `apps/stele/Cargo.lock`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json` | Lock-step `0.17.x → 0.18.0` via `python scripts/bump-version.py 0.18.0`.                                                                                                                    |

No changes to: `apps/stele/crates/stele-common/`, `apps/stele/crates/stele-cli/`, `apps/stylos/crates/stylos-common/`, `apps/stylos/crates/stylos-identity/`, `apps/steop/**`, `plugins/*/skills/*`, `apps/stele/Dockerfile`, `apps/stele/Cargo.toml` [dependencies] (zenoh dep unchanged — still `=1.9.0` with default features).

## 6. Edge Cases

| Scenario                                                                            | Behavior                                                                                                                                                                                                                                                                                                               |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Legacy `stylos.json5` with `transport.link.tls.*` present                           | **Parse error on load.** `StylosConfig` has no catch-all for unknown nested fields. User sees a Serde error at startup pointing at the `transport` key. Fix: delete the block (documented in §7 Migration).                                                                                                            |
| Legacy stele-server `config.toml` with `[stylos] no_quic = true`                    | **Silent ignore.** `StyloSettings` uses per-field `#[serde(default)]` and TOML is permissive with unknown keys at the struct level — the `no_quic` key is parsed and discarded. No warning, no error. Round-trip `save_settings` drops the key on next write.                                                          |
| Shell environment has `STELE_STYLOS_NO_QUIC=1`                                      | **Silent ignore.** The env var is no longer bound to any clap arg; it sits in the process environment unread. No warning at startup — matches how stele-server treats any other unknown env var.                                                                                                                       |
| Shell command passes `--stylos-no-quic`                                             | **Hard error.** Clap reports `unexpected argument '--stylos-no-quic'` and exits non-zero. Operators who had this in a systemd unit, Docker `-e`, or shell alias see the break on first upgrade. This is the load-bearing breaking change that justifies the minor bump.                                                |
| `stylos pub --no-quic …` via the stylos CLI                                         | **Hard error.** Same clap behavior as above. Dev scripts that still pass `--no-quic` after the upgrade break; this PRD's script cleanup (§4.5) keeps the shipped scripts in sync, but operator-local wrappers must be updated.                                                                                         |
| UDP port `31747` available, TCP port `31747` not available                          | `walk_available_port` walks forward (cap 8) until both TCP **and** UDP bind at the same port number. This is existing behavior unchanged; dropping QUIC does not change the walk semantics.                                                                                                                            |
| UDP-blocking network (corporate firewall drops UDP outbound)                        | UDP listener binds locally but peer discovery over UDP fails at the L3 layer. TCP listener remains reachable; zenoh falls back to TCP for inter-peer transport. Multicast scouting (`224.0.0.224:31746`) is independent and already documented as failing on such networks (PRD-022 §6).                               |
| Existing peer on the LAN still advertises `quic/0.0.0.0:31747`                      | Mixed-version mesh. Upgraded stele-server advertises `udp/` + `tcp/`; legacy peer advertises `quic/` + `tcp/`. Both can connect over TCP (the common subset). No data loss, just degraded connect-endpoint selection. Documented in §7 Migration as an upgrade-ordering note.                                           |
| zenoh-pico peer joins the mesh (future work)                                        | Pico connects over UDP or TCP depending on its build. Previously impossible on the QUIC endpoint; now possible on the UDP endpoint. This is the motivating case, not an edge case per se, but listed here for traceability to §3.3.                                                                                    |
| Operator still has `./certs/stylos-dev.*` files on disk                             | Files are ignored — no config path references them after the example-config rewrite. Left in place on disk, harmless. Migration (§7) mentions the files may be deleted.                                                                                                                                                 |

## 7. Migration

**Breaking changes in this PRD:**

1. **CLI surface:** `--stylos-no-quic` and `STELE_STYLOS_NO_QUIC` are removed from `stele-server`. `--no-quic` is removed from the `stylos` CLI. Operators who reference any of these in systemd units, Docker `-e` flags, shell aliases, or CI scripts must remove the reference before upgrading.
2. **Config file schema (stylos):** the `[stylos]`/`transport.link.tls.*` block in `stylos.json5` causes a parse error. Edit the file and delete the `transport: { ... }` block. No replacement block is needed — defaults take over.
3. **Config file schema (stele-server):** `[stylos] no_quic = …` in `config.toml` is silently ignored. Safe to leave in place; it drops on the next write. Cleanest path is to delete the key by hand.
4. **Transport advertisement:** a fresh upgrade publishes `udp/` + `tcp/` locators instead of `quic/` + `tcp/`. Operators who have **explicit** connect endpoints pinned at `quic/…` in any config must update them to `udp/…` or `tcp/…`. Mixed-version meshes fall back to TCP at the zenoh transport layer (§6).

**Non-breaking:**

- Port number (`31747`) unchanged.
- Multicast scout address (`224.0.0.224:31746`) unchanged.
- `stylos/<realm>/stele/<instance>/{heartbeat,info}` key-expr grammar unchanged.
- `/api/v1/health` response **schema** unchanged; only the example value in docs flips.
- MCP surface, REST mailbox/notify surface, auth-key behavior all unchanged.

**Upgrade ordering in a multi-host deployment:** upgrade one peer at a time. Because the post-upgrade peer still binds TCP at `31747`, it remains reachable from legacy peers on the common TCP subset during the rolling upgrade. No simultaneous-upgrade requirement.

**File cleanup (optional):** operators who previously ran `gen-dev-certs.sh` may delete the `./certs/stylos-dev.*` files and the `./certs/` directory. Left in place they are harmless — nothing reads them post-upgrade.

**Lock-step version bump:** `python scripts/bump-version.py 0.18.0`, then verify the five target files (§4.11) all show `0.18.0`. Flip the top-of-file `**Status:**` from `Proposed` to `Implemented (v0.18.0)` once §8 acceptance holds.

## 8. Testing

Manual smoke. No automated harness is added; the existing smoke scripts (minus `--no-quic` flags) remain the acceptance path.

### 8.1 Build verification

```bash
cd apps/stele
cargo build -p stele-server                                              # desktop + stylos
cargo build -p stele-server --no-default-features --features headless    # headless + stylos
cargo clippy -p stele-server --all-features

cd ../stylos
cargo build --workspace
cargo clippy --workspace --all-targets
```

All four invocations pass with no QUIC-related warnings (no dead-code warnings for the removed TLS structs, no missing-imports warnings).

### 8.2 Stele-server CLI negative test

```bash
cargo run -p stele-server -- --stylos-no-quic
# Expected: clap error, "unexpected argument '--stylos-no-quic'", exit 2.

STELE_STYLOS_NO_QUIC=1 cargo run -p stele-server
# Expected: server starts cleanly, env var is ignored (no warning, no error).
```

### 8.3 Stylos-cli negative test

```bash
cd apps/stylos
cargo run -p stylos-cli -- pub test/key msg --no-quic
# Expected: clap error, "unexpected argument '--no-quic'", exit 2.
```

### 8.4 Listen-endpoint advertisement

```bash
cargo run -p stele-server &
# Via a second host on the LAN:
cargo run -p stylos-cli -- get 'stylos/dev/stele/*/info'
# Expected: one JSON reply whose "listen_endpoints" field shows
#   ["udp/0.0.0.0:31747", "tcp/0.0.0.0:31747"]
# (or the walked-forward port) — no "quic/" prefix.
```

Note: per PRD-022 §4.9 the live `listen_endpoints` value in the queryable is still `Vec::new()` (zenoh 1.9 has no stable listener enum). This test validates the **docs-stated** example, which gets updated in `docs/stele/http-api.md`. The runtime value is verified via the stylos-cli get + subsequent socket probe:

```bash
nc -u -z -v 127.0.0.1 31747
# Expected: succeeds — UDP listener bound.
nc -z -v 127.0.0.1 31747
# Expected: succeeds — TCP listener bound.
```

### 8.5 Legacy-config parse error (stylos)

```bash
cd apps/stylos
cat > /tmp/legacy.json5 <<'EOF'
{ stylos: { realm: "dev", role: "cli", instance: "test" },
  zenoh:  { mode: "peer",
            transport: { link: { tls: { listen_private_key: "/dev/null" } } } } }
EOF
cargo run -p stylos-cli -- --config /tmp/legacy.json5 identity
# Expected: non-zero exit with a Serde error mentioning "transport".
```

### 8.6 Dev-script smoke

```bash
cd apps/stylos
./scripts/smoke-test.sh          # Rust↔Rust
./scripts/go-interop-test.sh     # Rust↔Go queryable
./scripts/go-pub-rust-sub-test.sh  # Go pub → Rust sub
# All three: exit 0, no "--no-quic" in the invocations (grep the scripts).
```

### 8.7 Health-endpoint regression

```bash
curl -s http://127.0.0.1:3100/api/v1/health | jq .stylos
# Expected: same shape as PRD-022 §4.10; listen_endpoints still [] at runtime
# per the known zenoh-1.9 limitation.
```

### 8.8 Shutdown cleanliness

Unchanged from PRD-022 §8.6 — opening a subscriber, Ctrl-C'ing stele-server, verifying the subscriber stops receiving heartbeats within ~5 s with no panic.

## 9. Open Questions

1. **UDP-only vs UDP + TCP listen set.** This PRD commits to UDP + TCP (§4.1) to preserve the existing `walk_available_port` dual-probe semantics and keep a TCP-interoperable fallback. Revisit if a future PRD introduces a dedicated UDP-only deployment profile (e.g. for a pico-heavy edge mesh).

2. **Transitive QUIC crates in `Cargo.lock`.** zenoh 1.9 ships QUIC as a default transport with no compile-time opt-out (§4.10). `quinn`, `rustls`, and the certificate-handling crates stay in the dependency closure unused. Acceptable for v0.18.0; flagged as cleanup candidates for the eventual zenoh 2.x upgrade PRD.

3. **zenoh-pico QUIC support claim.** §3.3 states pico does not support QUIC. The claim is user-supplied; primary-source verification against `eclipse-zenoh/zenoh-pico` upstream (via `context7` or GitHub) is recommended as a fast-follow, not a blocker for this PRD. A pico-TCP fallback would still benefit from this PRD's config-surface trim even if the QUIC claim turns out to be version-specific.

4. **UDP reliability posture.** zenoh's UDP transport is a datagram link with no built-in retransmit. Mailbox and notify still go over HTTP in v0.18.0 (PRD-020, PRD-022 §2); any future pub/sub-migrated surfaces must explicitly choose between UDP-datagram (lossy, cheap) and TCP (ordered, expensive) per key-expr. Out of scope for this PRD but worth noting for the consumer that first migrates a surface onto stylos pub/sub.

5. **Docs-only vs doc-and-PRD superseded-by.** PRD-019 §4.2 and §9, and PRD-022 §4.4/§4.9/§6 all state QUIC-specific facts that are no longer accurate. This PRD adds admonitions rather than rewriting the bodies (§4.8). Alternative: rewrite the bodies. Rejected here to preserve the decision-record archaeology, but flagged for reconsideration if the admonitions become unwieldy across future PRDs.
