# Stylos Architecture

Stylos is a zenoh-backed interconnect layer for the workspace. It exposes
pub/sub, query/queryable, and (later) storage primitives so Stele, Steop,
and future tooling share a single, push-capable signal bus instead of
stitching ad-hoc polling on top of HTTP.

## Process model

Each stylos participant is a single OS process that hosts one
`zenoh::Session` in **peer mode**. Peers auto-discover on the LAN via
multicast and form a flat mesh; no broker is required. A process may be
the `stylos` CLI, or a library consumer (future stele-server or steop
integration) that links `stylos-session` directly.

## Rust crate split

Five libraries plus one binary, with a strict DAG:

```
stylos-cli → stylos-session → stylos-transport → stylos-common
                            → stylos-identity  → stylos-common
                            → stylos-config    → stylos-common
```

- **stylos-common** — constants (multicast addr, default port, walk cap),
  `StylosError`, `Result<T>`.
- **stylos-identity** — `Realm`/`Role`/`Instance` newtypes with
  `[a-z0-9][a-z0-9-]*` validation and `StylosIdentity::root_key()` returning
  `stylos/<realm>/<role>/<instance>`.
- **stylos-config** — JSON5 loader (`StylosConfig::load`, `load_default`)
  mapping a stylos-flavoured document to an in-memory shape.
- **stylos-transport** — `listen_endpoints` locator builder,
  `walk_available_port` (TCP+UDP dual-bind check), `TlsPaths` bundle.
- **stylos-session** — `open_session(&cfg, &overrides) -> zenoh::Session`
  and `log_session_info` helper. This is the only crate that imports zenoh
  config surface details.
- **stylos-cli** — `stylos` binary with `pub`/`sub`/`get`/`queryable`/`identity`.

Downstream consumers pick the smallest crate they need: identity composers
without JSON5 overhead, or sessions without clap.

## Config construction

Zenoh 1.9.0's stable `Config` API exposes mutation exclusively through
`Config::insert_json5(key_path, value_json_str)`. `stylos-session` builds
the config entirely through this interface — mode, listen/connect
endpoints, scouting, and TLS are all written as small JSON5 fragments
keyed by slash-separated paths (e.g. `"scouting/multicast/address"`,
`"transport/link/tls/listen_private_key"`). This keeps stylos insulated
from any further `#[unstable]` surface churn in future zenoh minor
versions.

## Data flow

1. CLI parses args, loads `StylosConfig` (env → ./stylos.json5 → defaults).
2. `open_session` validates identity, walks ports if needed, constructs a
   `zenoh::Config` via `insert_json5`, and opens the session.
3. Subcommand runs pub/sub/get/queryable against key expressions.
4. Ctrl-C or deadline triggers a clean `session.close()`.
