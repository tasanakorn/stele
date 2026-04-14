# Stylos Pass B-0 Research — Verified Zenoh API Facts

Captured 2026-04-13 from primary sources (docs.rs/zenoh, github.com/eclipse-zenoh/zenoh[, -go]). Answers PRD-019 §9 open questions so the executor can write real code in Pass B-Rust / Pass B-Go without guessing.

## Rust zenoh crate

**Version status:** crate version tracks the zenoh repo workspace; the current zenoh-go binding is v1.9.0 (Apr 2026) targeting zenoh 1.x, so the Rust crate is in the zenoh **1.x** series. Pin the exact version in `Cargo.toml` when Pass B-Rust starts (check `cargo search zenoh` at execute time).

**Default features (zenoh/Cargo.toml):**
```
auth_pubkey, auth_usrpwd, transport_compression, transport_multilink,
transport_quic, transport_quic_datagram, transport_tcp, transport_tls,
transport_udp, transport_unixsock-stream, transport_ws
```

→ **`transport_quic` is default-on** (PRD §9 Q4 resolved). No special feature flag needed.

**Session entrypoint:**
```rust
use zenoh::Config;

#[tokio::main]
async fn main() {
    let session = zenoh::open(Config::default()).await.unwrap();
    // ...
    session.close().await.unwrap();
}
```

Loading config from JSON5: `Config::from_file("stylos.json5").unwrap()`.

**Programmatic config:**
```rust
let mut config = Config::default();
config.set_mode(Some("peer".parse().unwrap())).unwrap();
config.connect.endpoints.extend(["tcp/192.168.1.100:7447".parse().unwrap()]);
config.listen.endpoints.extend(["tcp/0.0.0.0:7448".parse().unwrap()]);
```

**Default listen (stock config):**
```
router: ["tcp/[::]:7447"], peer: ["tcp/[::]:0"]
```
→ peer mode does NOT listen on QUIC by default. Stylos must explicitly advertise `quic/...` and `tcp/...` locators.

**Default scouting multicast address: `224.0.0.224:7446`** (PRD §9 Q3 resolved — zenoh default port is 7446, our PRD overrides to 31746).

**Publisher:**
```rust
use zenoh::{bytes::Encoding, key_expr::KeyExpr};

let publisher = session.declare_publisher(&key_expr).await.unwrap();
publisher.put(buf).encoding(Encoding::TEXT_PLAIN).await.unwrap();
```

**Subscriber:**
```rust
let subscriber = session.declare_subscriber(&key_expr).await.unwrap();
while let Ok(sample) = subscriber.recv_async().await {
    println!("Received: {:?}", sample);
}
```

**Queryable (verbatim from `examples/z_queryable.rs`):**
```rust
let queryable = session
    .declare_queryable(&key_expr)
    .complete(complete)
    .await
    .unwrap();

while let Ok(query) = queryable.recv_async().await {
    match query.payload() {
        None => println!(">> Received Query '{}'", query.selector()),
        Some(p) => {
            let s = p.try_to_string().unwrap_or_else(|e| e.to_string().into());
            println!(">> Received Query '{}' with payload '{}'", query.selector(), s);
        }
    }
    query.reply(key_expr.clone(), payload.clone()).await.unwrap();
}
```

**Get/Query:**
```rust
let replies = session.get("key/expression").await.unwrap();
while let Ok(reply) = replies.recv_async().await {
    println!(">> Received {:?}", reply.result());
}
```

**Scouting (LAN peer discovery):**
```rust
use zenoh::{config::WhatAmI, scout};

let receiver = scout(WhatAmI::Peer | WhatAmI::Router, Config::default())
    .await.unwrap();
while let Ok(hello) = receiver.recv_async().await {
    println!("Found: {hello}");
}
receiver.stop();
```

**Session info:**
```rust
let info = session.info();
info.zid().await;
info.routers_zid().await.collect::<Vec<_>>();
info.peers_zid().await.collect::<Vec<_>>();
```

**Log init:** `zenoh::init_log_from_env_or("error");`

**Async runtime:** `#[tokio::main]` required (zenoh APIs are all `async`).

## Go zenoh-go binding

**Module:** `github.com/eclipse-zenoh/zenoh-go` · latest v1.9.0 (Apr 10, 2026). Active maintenance.

**Import paths:**
```go
import (
    "github.com/eclipse-zenoh/zenoh-go/zenoh"
    "github.com/eclipse-zenoh/zenoh-go/examples/utils"  // example helper only
)
```

**Native dependency (load-bearing):** requires **zenoh-c** installed on the system, built with `-DZENOHC_BUILD_WITH_UNSTABLE_API=ON`. zenoh-go is a CGO wrapper around zenoh-c, not a pure-Go binding. This means Pass B-Go builds will fail on any host without zenoh-c installed — a significant environmental constraint that must be documented and handled in the POC instructions.

**Session:**
```go
session, err := zenoh.Open(config, nil)
defer session.Drop()
```

**Publisher:**
```go
pub, err := session.DeclarePublisher(keyexpr, nil)
putOpts := zenoh.PublisherPutOptions{}
pub.Put(zenoh.NewZBytesFromString(message), &putOpts)
```

**Subscriber (callback-style):**
```go
func dataHandler(sample zenoh.Sample) {
    fmt.Printf("%s '%s': '%s'\n", kindToStr(sample.Kind()), sample.KeyExpr().String(), sample.Payload().String())
}
sub, err := session.DeclareSubscriber(keyexpr, zenoh.Closure[zenoh.Sample]{Call: dataHandler}, nil)
defer sub.Drop()
```

**KeyExpr:**
```go
keyexpr, err := zenoh.NewKeyExpr(args.keyexpr)
```

**Init:** `zenoh.InitLoggerFromEnvOr("error")`

**Peer mode (PRD §9 Q1):** zenoh-go wraps zenoh-c, which inherits the full zenoh protocol including peer mode. The example uses `zenoh.Open(config, nil)` with the same JSON5 config shape as Rust — default is peer. **Peer mode is supported**; the earlier concern about client-only was based on outdated binding state, now resolved by v1.9.0. The PRD's contingency ("Go peer attaches to Rust router as a client") is no longer needed at the architectural level; it remains a fallback only if specific QUIC or TLS features turn out not to be plumbed through zenoh-c's public API.

## Config file format (JSON5)

Stylos's PRD §4.5 JSON5 example is structurally compatible with zenoh 1.x `Config::from_file()` — the `zenoh` sub-object's fields (`mode`, `connect.endpoints`, `listen.endpoints`, `scouting.multicast.*`, `scouting.gossip.*`, `transport.link.tls.*`) all appear in the stock `DEFAULT_CONFIG.json5`. The outer `stylos` wrapper needs to be stripped before the inner `zenoh` block is passed to `Config::from_file`, or Stylos needs a custom loader that reads our shape and builds a `Config` programmatically. Recommend the latter: keeps the Stylos identity fields (realm/role/instance) first-class in our config surface.

**TLS default fields** (from stock DEFAULT_CONFIG.json5 — note PRD §4.5 uses different key names):
```
tls: {
    root_ca_certificate: null,
    listen_private_key: null,
    listen_certificate: null,
    enable_mtls: false,
    connect_private_key: null,
    connect_certificate: null,
    verify_name_on_connect: true,
    close_link_on_expiration: false,
}
```

**PRD §4.5 vs upstream field-name mismatch:** PRD uses `server_private_key` / `server_certificate`, upstream uses `listen_private_key` / `listen_certificate`. Pass B-Rust must normalize to upstream names before passing to zenoh, OR the PRD's JSON5 example must be corrected. Flag as a resolved-but-requires-code-choice item for the architect.

## Summary of PRD §9 open questions resolved

| Q | Question | Answer |
| - | -------- | ------ |
| 1 | Zenoh-go peer mode status | **Supported** — v1.9.0 wraps zenoh-c full protocol; config drives mode. |
| 3 | Default scouting port | **7446** (stock default; stylos overrides to 31746 per PRD). |
| 4 | `transport_quic` feature flag | **Default-on** in current zenoh Cargo.toml. No extra `--features` needed. |

Q2 (zenoh-ts transport story) remains out of scope per PRD non-goals.

## Implications for Pass B plan

1. **Pass B-Rust is safe to proceed.** All Rust APIs above are verified from primary source. Pin zenoh version at execute time with `cargo add zenoh` and record exact version in Cargo.toml.

2. **Pass B-Go has an environmental blocker** — zenoh-c must be pre-installed. This means:
   - `go build ./...` will fail on CI / clean machines without zenoh-c.
   - Docs must include a "how to install zenoh-c" section.
   - Test-harness on macOS needs `brew install` or source-build instructions.
   - Consider whether Pass B-Go should be gated on zenoh-c install or use a build tag to skip the zenoh-dependent parts.

3. **JSON5 config normalization.** Two realistic paths:
   - (a) Stylos config file embeds a raw zenoh Config as a nested `zenoh:` block; Stylos loader extracts that block, writes it to a tempfile, and calls `Config::from_file(tempfile)`. Hacky.
   - (b) Stylos config file contains only stylos-level fields + a small stylos-specific transport summary; stylos builds a `Config` programmatically via `set_mode` / `listen.endpoints.extend` / etc. Cleaner. Recommended.

4. **TLS field-name correction needed in PRD §4.5 example.** Update to upstream names (`listen_private_key`, `listen_certificate`, `root_ca_certificate`) before the example config in `apps/stylos/stylos.example.json5` references them in Pass B-Rust.

## Sources

- https://github.com/eclipse-zenoh/zenoh/blob/main/examples/examples/z_pub.rs
- https://github.com/eclipse-zenoh/zenoh/blob/main/examples/examples/z_sub.rs
- https://github.com/eclipse-zenoh/zenoh/blob/main/examples/examples/z_get.rs
- https://github.com/eclipse-zenoh/zenoh/blob/main/examples/examples/z_queryable.rs
- https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5
- https://github.com/eclipse-zenoh/zenoh/blob/main/zenoh/Cargo.toml
- https://github.com/eclipse-zenoh/zenoh-go (v1.9.0, Apr 10 2026)
- https://github.com/eclipse-zenoh/zenoh-go/blob/main/examples/z_sub/z_sub.go
- https://github.com/eclipse-zenoh/zenoh-go/blob/main/examples/z_pub/z_pub.go
- https://pkg.go.dev/github.com/eclipse-zenoh/zenoh-go (v1.9.0)
- Context7 library `/eclipse-zenoh/zenoh` (llms.txt distillation)
