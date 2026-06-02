# PRD-027 — Postal mailbox: decentralized per-node delivery with attention routing

- **Status:** Implemented v0.20.0
- **Target version:** workspace v0.20.0 (minor + documented migration)
- **Scope:** `apps/stele/crates/stele-cli/` (new `mail` feature, `commands/mail.rs`, `stylos_client.rs`, `config.rs`, `main.rs`), `apps/stele/crates/stele-server/src/stylos_module.rs` (mailbox queryables + delivery/retry worker + heartbeat listener), `apps/stele/crates/stele-server/src/db.rs` (3 tables + access fns), `apps/stele/crates/stele-server/src/steop_api.rs` (attention envelope on the retained REST mailbox types), `docs/stylos/addressing.md`, `docs/steop/DESIGN.md`, `docs/stele/http-api.md`, `docs/README.md`, lock-step version bumps (`apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json`)
- **Author:** Tasanakorn (design) + Claude Code (PRD authoring)

> **Extends [PRD-020](prd-020-steop-local-backend.md) (v0.16.0) and [PRD-022](prd-022-stylos-in-stele-server.md) (v0.17.0).** PRD-020 narrowed the server-side steop surface to `steop_mailbox` + notify; PRD-022 embedded a zenoh peer inside stele-server under `stylos/{realm}/stele/{instance}/*`. This PRD does **not** supersede either. It builds the next mesh leaf — `.../mailbox/*` queryables and an origin-spooled delivery worker — on top of PRD-022's session, and adds a new mailbox surface alongside (not replacing) the REST routes the steop Go client still uses.

> **Implementation status (v0.20.0).** Landed: the three tables, all seven `mailbox/*` queryables, `mail_uid` ULID dedupe, the `stele mail` CLI (default-on `mail` feature), the delivery/retry worker, and the heartbeat-reachability map. Single-host behaviour is covered by `apps/stele/scripts/smoke-mail.py` (10 checks green). Deviations from the spec as written, all reflected above: the inbox upsert fn is `mailbox_inbox_upsert` (not `..._send`); the CLI peer opens in **`client`** mode, not `peer` (a `peer` session lingers ~10s on close — §4.13); `list` filters by the **caller-supplied** aliases, registry only as fallback (§4.6); and both the node's `mailbox_host` and the CLI honour **`STELE_HOST` > hostname** (steop's precedence), with the project segment detected from `CLAUDE_PROJECT_DIR` only (no `PWD` fallback). **Pending validation:** cross-node delivery (outbox spool → mesh `deliver` → dead-letter) compiles but is unexercised — it needs a second node on the same realm.

---

## 1. Goals

1. **Make each machine a post office.** One stele node per host (1:1 host↔node). A mailbox is a "home" addressed by `host:project_dir`, and mail for that home is stored at the **destination node's** stele-server SQLite (the DB that already holds memories + `steop_mailbox`). No central hub, no `steop.db`.
2. **Replace the composite-id 3rd segment with an `attention` envelope.** The old `:UUID` / `:USER` third segment (PRD-020 identity grammar) is dropped from the routing identity. Mail carries an optional `attention` field (agent alias/name, the real-world "Attn: Claude"). `null` = household mail visible to every reader of the home; reserved `*` = explicit household broadcast.
3. **Add a `stele mail` CLI surface** — `send | list | read | archive | get | outbox | register` — in stele-cli as a **default-on** `mail` cargo feature, so a plain `cargo install`/`cargo build` ships the mail commands; the lean ureq-only build remains available via `--no-default-features`. The mail path talks to the **local node** over stylos/zenoh queryables, not REST.
4. **Deliver across the mesh with durable origin spooling.** When the destination node is offline, the **origin** node spools the message in its own outbox and retries with exponential backoff, skipping hosts with no recent heartbeat, dead-lettering after a TTL/attempt cap. No relay, no store-and-forward hub.
5. **Exactly-once at rest, at-least-once on the wire.** The origin mints a stable global `mail_uid` (ULID); the destination dedupes on it (`UNIQUE`). A duplicate `deliver` returns the existing `message_id` with status `duplicate`.

## 2. Non-goals

- **No central relay or hub.** Delivery is strictly node→node. An offline destination is handled by the origin's outbox, never by a third party.
- **No real-time push to recipients.** Mail lands in the destination's SQLite; readers **poll** via `stele mail list`. No subscription/notify fan-out to end clients in this PRD.
- **No memory or knowledge-graph replication.** Only mailbox rows cross the mesh. Memories, scopes, tags, and the graph stay node-local.
- **No rework of the steop Go mailbox client.** `apps/steop/internal/client/mailbox.go` and its REST path stay **legacy/unchanged**. The new postal surface is additive; the Go client keeps calling `POST /api/v1/steop/mailbox.*`.
- **No new auth/ACL/TLS.** The mesh inherits PRD-022/PRD-023 transport posture (UDP+TCP on 31747, no TLS). Mailbox queryables carry no per-message authorization beyond network reachability.
- **No stylos crate changes.** New leaves and the transient-peer CLI use the existing `stylos::open_session` surface (pinned git tag v0.2.1). `apps/stylos/**` is untouched.

## 3. Background & Motivation

### 3.1 Current state

The cross-agent mailbox today is a single REST surface on stele-server, consumed only by the steop Go binary:

- Table `steop_mailbox` lives in stele-server's main SQLite schema block (`apps/stele/crates/stele-server/src/db.rs:141-155`): `message_id INTEGER PK AUTOINCREMENT, from_id, to_id, subject, message_type, meta, payload, created_at, status`. Indexed on `(to_id, status, created_at)` and `(from_id, created_at)`. Row struct `SteopMailboxRow` (`db.rs:1209-1222`) renames `from_id→from`, `to_id→to` over serde.
- Status lifecycle is `NEW | READ | ARCHIVE` (note: `ARCHIVE`, not `ARCHIVED`), validated at `steop_api.rs:252`; transitions go through `MailboxTransition` (`db.rs:1340-1419`) — `read` only from `NEW`, `archive` from `NEW | READ`, illegal → `409`.
- DB fns `steop_mailbox_send/_get/_list/_read/_update_meta/_archive` (`db.rs:1252-1419`); helpers `steop_now`, `steop_parse_json`, `steop_json_merge` (`db.rs:1224-1238`).
- HTTP routes under `/api/v1/steop` (`steop_api.rs:17-28`): `steop.mailbox.{send,list,get,read,archive,update_meta}` + `steop.notify`. Request shapes at `steop_api.rs:127-182`.
- **Identity is a closed-set composite id.** Both enforcers — Rust `steop_api.rs:53-95` (`Principal::{Project, Session(uuid), User}`, rejecting any non-UUID/non-`USER` 3rd segment at `:86-88`, `USER_LITERAL` at `:51`) and Go `apps/steop/internal/store/identity.go:29-62` (UUID regex at `:9` or `USER`, else error at `:61`) — accept only `host:project_dir`, `host:project_dir:UUID`, or `host:project_dir:USER`. Documented in CLAUDE.md "Steop RPC identity".
- The mesh exists (PRD-022): stele-server opens a zenoh session and serves `stylos/{realm}/stele/{instance}/{heartbeat,info}` (`stylos_module.rs`). `<instance>` is a **normalized** hostname (`derive_instance` `stylos_module.rs:83-116`, grammar `[a-z0-9][a-z0-9-]*` via `is_valid_instance` `:118-125`), falling back to `stele-<short-zid>`. The `info` queryable replies with a `serde_json::json!` blob assembled inline (`stylos_module.rs:230-239`); the heartbeat is **published** (`:188-206`) but nothing subscribes to it today.
- **stele-cli is HTTP-only.** clap `Commands` enum + dispatch at `main.rs:308-497`; `SteleClient` is a sync ureq client (`client.rs`); deps are ureq with **no zenoh and no `[features]` section** (`stele-cli/Cargo.toml`). `Profile { server_url, auth_key, host }` (`config.rs:11-18`) has **no zenoh endpoint field**; `main()` is **sync** (`main.rs:266`). Host comes from `gethostname` (`config.rs:86-92`). `ulid = "1"` is **already a stele-server dep** (`stele-server/Cargo.toml:28`) and used for entity/observation/relation ids (`db.rs:683,703,873`).

### 3.2 Why decentralize the mailbox now

- **A node already exists per host.** PRD-022 put a long-lived zenoh peer on every stele-server. The home's `host` segment and the node's identity are the same machine — storing a home's mail at that home's own node is the natural shape, and it removes the single-DB bottleneck where one server holds every host's mailbox.
- **The postal metaphor maps cleanly.** `host:project_dir` is a street address; the destination node is the post office that holds the letter; `attention` is "Attn: <name>" on the envelope. Senders address a home by name and may never have seen it — the destination auto-creates the home row on first delivery, exactly like mailing a house that was just built.
- **Durability belongs at both ends.** The origin keeps an outbox until delivery is confirmed; the destination keeps the inbox until the recipient reads/archives. Neither end depends on a hub being up.

### 3.3 Why attention replaces the 3rd id segment

The closed-set `:UUID`/`:USER` segment overloaded the *address* with *recipient selection*. A session UUID minted on one host is meaningless as a routing key on another, and `USER` was a sentinel bolted onto the same grammar. Splitting them — `host:project_dir` is the **home address** (still valid under both existing parsers), `attention` is a separate envelope field — keeps the address stable and globally meaningful while making recipient selection a first-class, case-insensitive match against agent aliases.

## 4. Design

### 4.1 Postal model and addressing

A **home** is `host:project_dir` — a 2-segment id under the existing grammar. Mail for a home is stored at the node whose host equals the home's `host` segment. The envelope adds:

| Field         | Example                       | Meaning                                                              |
| ------------- | ----------------------------- | ------------------------------------------------------------------- |
| `to_host`     | `vm-02`                       | Destination machine → resolves to a node `<instance>` (§4.2)        |
| `to_project`  | `/projects/project_a`         | Project dir at the destination home                                 |
| `attention`   | `claude` / `null` / `*`       | Recipient selector: alias match (case-insensitive), household, broadcast |
| `from`        | `vm-01:/repos/foo` (or `…:alias`) | Origin home address; symmetric — a reply re-addresses `from`     |

`attention` matching at `list` time: a row is visible to a caller when `attention IS NULL` (household), or `attention = '*'` (broadcast), or `lower(attention)` is in the caller's alias set (§4.6). Reserved `*` is never a valid alias.

**Alternative considered:** keep the `:UUID`/`:USER` 3rd segment and route on it. Rejected: a session UUID is host-local and meaningless as a cross-node routing key, and it conflates address with recipient selection.

### 4.2 Host → instance resolution (the load-bearing gap)

The mailbox `to_host` is a raw hostname (`gethostname`, e.g. `MacBook.local`) and is **not** constrained to the stylos instance grammar `[a-z0-9][a-z0-9-]*`. The deliver key `stylos/{realm}/stele/{dest-instance}/mailbox/deliver` needs a `<instance>`, not a raw host. These are two namespaces today (`derive_instance` `stylos_module.rs:83-116` normalizes; the mailbox host does not).

**Decision: nodes publish their mailbox-host claim; senders resolve via the existing `info` queryable.** Each node already serves `stylos/{realm}/stele/{instance}/info` (PRD-022 §4.9). This PRD adds one field to that JSON blob: `mailbox_host` (the node's raw `gethostname` value, §4.11). To deliver to `to_host`:

1. The origin issues a wildcard GET on `stylos/{realm}/stele/*/info`, collecting `{instance, mailbox_host}` pairs.
2. It selects the instance whose `mailbox_host` equals `to_host` (case-insensitive). That instance is the deliver key segment.
3. The mapping is **not** stored long-term in the outbox row (`dest_instance` is re-resolved per attempt) so a node that re-instances keeps receiving mail.

No new normalization contract is imposed on hostnames — the node *declares* its own host↔instance binding, and the sender trusts the declaration. If no live node claims `to_host`, the message stays `QUEUED` and the retry worker skips it (no heartbeat seen for that host, §4.8).

**Alternative considered:** define a canonical `host → instance` normalization both sides apply (lowercase, non-`[a-z0-9-]`→`-`, cap). Rejected: it collides (`MacBook.local` and `macbook-local` both normalize to `macbook-local`) and forces senders to guess a node's instance without confirmation; the published-claim approach is collision-free and self-correcting.

### 4.3 Mesh key-expr leaves

Seven new leaves under the PRD-022 namespace `stylos/{realm}/stele/{instance}/mailbox/`:

| Leaf       | Caller            | Purpose                                                          |
| ---------- | ----------------- | --------------------------------------------------------------- |
| `send`     | local CLI peer    | Accept a new outbound message; self-store + spool remote        |
| `list`     | local CLI peer    | Return inbox rows visible to caller's aliases / household        |
| `read`     | local CLI peer    | `NEW → READ` transition on a `message_id`                        |
| `archive`  | local CLI peer    | `NEW|READ → ARCHIVE` transition                                  |
| `get`      | local CLI peer    | Fetch one message by `message_id`                               |
| `outbox`   | local CLI peer    | Inspect this node's pending/failed outbound spool               |
| `deliver`  | **remote node**   | Node→node inbound delivery (the only cross-node leaf)           |

`deliver` is the sole leaf a *remote* node calls. The other six are the **local client API** — only the host's own transient CLI peer issues them. The `register` CLI command (§4.6) writes through the `send`/`list` peer using the same `mailbox/list` path (it carries an `aliases[]` field; no separate leaf). All are zenoh **queryables** (request/reply), JSON-encoded (`Encoding::APPLICATION_JSON`), declared in `stylos_module.rs` next to the existing `info` queryable, following the queryable+task pattern at `stylos_module.rs:208-256`.

**Uniform reply envelope.** Every leaf replies with a JSON object. Success carries leaf-specific fields plus no `ok` key needed for the success case is **avoided**; instead **all** replies use:

```json
{ "ok": true,  ... }                                  // success, leaf-specific fields alongside
{ "ok": false, "error": "<machine-code>", "reason": "<human>" }   // failure
```

Machine codes (closed set): `bad_request`, `not_found`, `conflict`, `internal`, `undeliverable`. The CLI maps these to exit codes (§4.5). A `recv` error or empty reply on the wire is treated as `internal`.

### 4.4 Deliver contract (node → node)

- **Key:** `stylos/{realm}/stele/{dest-instance}/mailbox/deliver`
- **Transport:** zenoh GET (origin) → reply (destination).
- **Request (JSON):** `{ mail_uid, to_project, attention?, from, subject?, message_type?, meta?, payload? }`
- **Reply (JSON):** `{ ok: true, message_id, status: "stored" | "duplicate" }` or `{ ok: false, error, reason }`.

Rust structs (in `stylos_module.rs`, `serde`):

```rust
#[derive(serde::Deserialize)]
struct DeliverReq {
    mail_uid: String,
    to_project: String,
    #[serde(default)] attention: Option<String>,
    from: String,
    #[serde(default)] subject: Option<String>,
    #[serde(default)] message_type: Option<String>,
    #[serde(default)] meta: Option<serde_json::Value>,
    #[serde(default)] payload: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct DeliverReply { ok: bool, message_id: i64, status: String }
```

Destination behavior: lock the `DbPool`, call `db::mailbox_inbox_send` (§4.7) which `INSERT … ON CONFLICT(mail_uid) DO NOTHING` then `SELECT message_id, status` by `mail_uid`. Fresh insert → `{ ok:true, message_id, status:"stored" }`. Pre-existing `mail_uid` → `{ ok:true, message_id, status:"duplicate" }` (idempotent — the origin marks `DELIVERED` either way). Any SQLite error → `{ ok:false, error:"internal", reason }`, leaving the origin row `QUEUED` for retry. The home row is **auto-created** implicitly — there is no separate homes table; the first delivered row for a `(to_project)` is the home (mail-a-house-never-seen).

### 4.5 Client API (local node, via queryables)

The transient CLI peer issues these against its **own** local node. Request/reply structs live in `stele-cli/src/stylos_client.rs` (mirrored from the server structs above). Each reply is the §4.3 envelope.

**`send`** — request `{ to_host, to_project, attention?, subject?, message_type?, meta?, payload?, from }` (`from` is composed by the CLI, §4.12). Node mints `mail_uid` (ULID). If `to_host` (case-insensitive) == this node's own `mailbox_host` (§4.11), store directly into inbox (status `NEW`) and reply `{ ok:true, mail_uid, message_id, status:"delivered" }`. Otherwise insert an outbox row (`QUEUED`, `next_attempt_at = now`) and reply `{ ok:true, mail_uid, status:"queued" }` (fire-and-forget; the worker delivers asynchronously).

**`list`** — request `{ project_dir, aliases?: string[], status?: string[] }`. `aliases` are the caller's roles (lowercased server-side); when present they are also upserted into `mailbox_agent_alias` for `project_dir` (this is the `register` mechanism, §4.6). Reply `{ ok:true, messages: [InboxRow…] }`. SQL `WHERE` (§4.6) selects rows for `to_project = project_dir` whose `attention` matches the alias set / household / broadcast, filtered by `status[]` (default `['NEW','READ']`).

**`outbox`** — request `{ status?: string[] }` (default `['QUEUED','DEAD']`). Reply `{ ok:true, rows: [OutboxRow…] }` — `mail_uid, to_host, to_project, status, attempts, next_attempt_at, last_error, remote_message_id, created_at, delivered_at`.

**`read` / `archive`** — request `{ message_id }`. Reply `{ ok:true, message_id, status }` on success; `{ ok:false, error:"not_found" }` or `{ ok:false, error:"conflict", reason }` per the existing `MailboxTransition` (`db.rs:1246`) reused verbatim against `mailbox_inbox`.

**`get`** — request `{ message_id }`. Reply `{ ok:true, message: InboxRow }` or `{ ok:false, error:"not_found" }`.

**`register`** — request `{ project_dir, aliases: string[] }` on the `mailbox/list` key with an empty status filter and a `register_only: true` flag; upserts aliases and replies `{ ok:true, registered: aliases }` without returning messages. (Keeps one leaf; §4.6.)

**Per-leaf request/reply summary** (key-expr is `stylos/{realm}/stele/{local-instance}/mailbox/<leaf>`):

| Leaf      | Request JSON                                                                                  | Success reply JSON                                            |
| --------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `send`    | `{to_host, to_project, attention?, subject?, message_type?, meta?, payload?, from}`           | `{ok:true, mail_uid, message_id?, status}`                   |
| `list`    | `{project_dir, aliases?:[str], status?:[str]}`                                                | `{ok:true, messages:[InboxRow]}`                             |
| `register`| `{project_dir, aliases:[str], register_only:true}` (on `list` key)                            | `{ok:true, registered:[str]}`                                |
| `read`    | `{message_id:i64}`                                                                            | `{ok:true, message_id, status:"READ"}`                       |
| `archive` | `{message_id:i64}`                                                                            | `{ok:true, message_id, status:"ARCHIVE"}`                    |
| `get`     | `{message_id:i64}`                                                                            | `{ok:true, message:InboxRow}`                                |
| `outbox`  | `{status?:[str]}`                                                                             | `{ok:true, rows:[OutboxRow]}`                                |
| `deliver` | `{mail_uid, to_project, attention?, from, subject?, message_type?, meta?, payload?}`          | `{ok:true, message_id, status:"stored"\|"duplicate"}`        |

Error reply for every leaf: `{ ok:false, error:<code>, reason?:<human> }`, codes from §4.3.

### 4.6 Attention registry & alias resolution

**Decision: stateful registry, but populated as a side-effect of `list` (option (a) merged into the read path), not a separate command path.** A caller passes its `aliases[]` on `list`; the node upserts `(project_dir, alias, last_seen=now)` then runs the filter **using exactly those caller-supplied aliases** (the caller asserts its own identity on each call). Only when the caller supplies **no** aliases does the node fall back to the project's persisted registry. `stele mail register` is sugar that calls `list` with `register_only:true`. This satisfies "agents opt into aliases that survive reconnects" (rows persist, and a bare `list` resolves them) while keeping `attention` a per-caller assertion — registering `claude` for a project does **not** leak `claude`-addressed mail to a different caller listing the same project with `--alias other`.

**Alternative considered:** fully stateless — pass `--as <alias>` inline on every `list` and drop `mailbox_agent_alias` entirely. Rejected: it forces every caller (including future non-CLI readers and the `register` UX) to remember and re-supply its full role set on every call; a persisted registry lets an agent register once and poll bare thereafter, and gives `register` something durable to write.

**`last_seen` upkeep / expiry:** refreshed to `now` on every `list`/`register` that names the alias. No background GC in this PRD; a stale alias simply keeps matching (a role outliving its agent is harmless — it only widens the household view). A future PRD may prune `last_seen < now - 30d`; noted, not implemented.

**Resolution SQL** (run inside `list`, after the alias upsert):

```sql
-- caller_aliases = the request's aliases[]  (or, when empty:
--   SELECT alias FROM mailbox_agent_alias WHERE project_dir = ?1)
SELECT message_id, mail_uid, to_project, attention, from_addr, subject,
       message_type, meta, payload, created_at, status
FROM mailbox_inbox
WHERE to_project = ?1
  AND ( attention IS NULL
        OR attention = '*'
        OR lower(attention) IN ( <one ?n placeholder per caller alias> ) )
  AND status IN ( <one ?n placeholder per requested status> )
ORDER BY created_at ASC
LIMIT 1000;
```

When the caller supplies **no** aliases and the project registry is also empty, the `lower(attention) IN (…)` clause is omitted entirely (only household + broadcast rows are visible). Aliases are lowercased on write and on the inline comparison so matching is case-insensitive by construction; `*` is rejected as an alias at upsert (§4.10).

### 4.7 Storage: three tables (stele-server SQLite)

All three are added to the **main schema block** at `db.rs:141` (the `conn.execute_batch("…")` that already creates `steop_mailbox`), **not** `ensure_steop_schema` (`db.rs:162-178`, which only `DROP`s). The existing `steop_mailbox` is **reshaped** into `mailbox_inbox`; `SteopMailboxRow` (`db.rs:1209`) is renamed `MailboxRow` and gains `mail_uid` + `attention`, renames `from_id→from_addr` (serde `rename="from"`), drops `to_id` in favor of `to_project`. To force a clean recreate, add `DROP TABLE IF EXISTS steop_mailbox;` and the three new table names are **not** dropped (they are created with `IF NOT EXISTS` and persist).

**1. `mailbox_inbox`** (homes + delivered mail):

```sql
CREATE TABLE IF NOT EXISTS mailbox_inbox (
    message_id   INTEGER PRIMARY KEY AUTOINCREMENT,  -- local, per-node
    mail_uid     TEXT    NOT NULL UNIQUE,             -- origin-minted ULID, dedupe key
    to_project   TEXT    NOT NULL,
    attention    TEXT,                                -- NULL = household
    from_addr    TEXT    NOT NULL,                    -- "host:project_dir[:alias]"
    subject      TEXT    NOT NULL DEFAULT '',
    message_type TEXT    NOT NULL DEFAULT 'NOTE',
    meta         TEXT    NOT NULL DEFAULT '{}',
    payload      TEXT    NOT NULL DEFAULT '{}',
    created_at   TEXT    NOT NULL,
    status       TEXT    NOT NULL DEFAULT 'NEW'       -- NEW | READ | ARCHIVE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_mailbox_inbox_uid
    ON mailbox_inbox(mail_uid);
CREATE INDEX IF NOT EXISTS idx_mailbox_inbox_to
    ON mailbox_inbox(to_project, status, created_at);
```

(`mail_uid` is both `UNIQUE` inline and covered by an explicit unique index for the `ON CONFLICT(mail_uid)` upsert target and dedupe lookups.)

**2. `mailbox_outbox`** (origin spool):

```sql
CREATE TABLE IF NOT EXISTS mailbox_outbox (
    mail_uid          TEXT    PRIMARY KEY,            -- same ULID minted at send
    to_host           TEXT    NOT NULL,
    to_project        TEXT    NOT NULL,
    attention         TEXT,
    from_addr         TEXT    NOT NULL,
    subject           TEXT    NOT NULL DEFAULT '',
    message_type      TEXT    NOT NULL DEFAULT 'NOTE',
    meta              TEXT    NOT NULL DEFAULT '{}',
    payload           TEXT    NOT NULL DEFAULT '{}',
    status            TEXT    NOT NULL DEFAULT 'QUEUED', -- QUEUED | DELIVERED | DEAD
    attempts          INTEGER NOT NULL DEFAULT 0,
    next_attempt_at   TEXT    NOT NULL,
    last_error        TEXT,
    remote_message_id INTEGER,
    created_at        TEXT    NOT NULL,
    delivered_at      TEXT
);
CREATE INDEX IF NOT EXISTS idx_mailbox_outbox_due
    ON mailbox_outbox(status, next_attempt_at);
```

**3. `mailbox_agent_alias`** (attention registry):

```sql
CREATE TABLE IF NOT EXISTS mailbox_agent_alias (
    alias        TEXT    NOT NULL,                    -- one row per alias, lowercased
    project_dir  TEXT    NOT NULL,
    last_seen    TEXT    NOT NULL,
    PRIMARY KEY (project_dir, alias)
);
```

(Dropped the `agent_id` column from the earlier draft: §4.6 keys the registry on `(project_dir, alias)` only — there is no per-agent identity in the postal model, only roles. Upsert is `INSERT … ON CONFLICT(project_dir, alias) DO UPDATE SET last_seen = excluded.last_seen`.)

DB access fns added near `db.rs:1252`: `mailbox_inbox_send` (upsert-by-uid, returns `(message_id, status)`), `mailbox_inbox_get`, `mailbox_inbox_list`, `mailbox_inbox_read`, `mailbox_inbox_archive` (the last two reuse `MailboxTransition`); `mailbox_outbox_enqueue`, `mailbox_outbox_due` (returns `QUEUED` rows with `next_attempt_at <= now`), `mailbox_outbox_mark` (set status/attempts/next_attempt_at/last_error/remote_message_id/delivered_at), `mailbox_outbox_list`; `mailbox_alias_upsert(project_dir, &[alias])`, `mailbox_alias_list(project_dir)`.

### 4.8 Delivery / retry worker

A periodic task in stele-server, spawned alongside the heartbeat/info tasks in `stylos_module.rs::start` and owned by `StylosHandle` (a new `delivery_task: JoinHandle<()>` field next to `heartbeat_task` / `queryable_task` at `stylos_module.rs:60-65`, aborted+awaited in `shutdown` at `:68-76`), sharing the same `CancellationToken`. It mirrors the heartbeat worker template (`stylos_module.rs:188-206`): `tokio::time::interval` with `MissedTickBehavior::Skip`, `tokio::select!` on `ct.cancelled()`. It needs the `DbPool` (for the outbox) and the `Arc<zenoh::Session>` (to issue `deliver` GETs) — both cloned into the task.

Constants (module consts in `stylos_module.rs`):

| Name                | Value            | Meaning                                              |
| ------------------- | ---------------- | ---------------------------------------------------- |
| `WORKER_TICK`       | `5s`             | interval between outbox sweeps                       |
| `BACKOFF_BASE`      | `5s`             | first retry delay                                    |
| `BACKOFF_MULT`      | `2`              | exponential multiplier                               |
| `BACKOFF_CAP`       | `300s` (5 min)   | max single delay                                     |
| `MAX_ATTEMPTS`      | `50`             | dead-letter after this many failures                 |
| `OUTBOX_TTL`        | `7 days`         | dead-letter once `now - created_at > TTL`            |
| `DELIVER_TIMEOUT`   | `500ms`          | per-attempt GET timeout — localhost/LAN target (bound on a stuck reply) |
| `HEARTBEAT_FRESH`   | `15s`            | a host is "reachable" if seen within this window     |

`backoff(attempts) = min(BACKOFF_CAP, BACKOFF_BASE * BACKOFF_MULT^(attempts-1))`, computed as a `Duration`, added to `now`.

Each tick:

1. `mailbox_outbox_due(now)` → `QUEUED` rows with `next_attempt_at <= now`, `ORDER BY next_attempt_at`.
2. For each due row, look up `to_host` in the **reachability map** (§4.9). If the host has no heartbeat within `HEARTBEAT_FRESH`, **skip** (do not increment `attempts`, do not change `next_attempt_at`). Otherwise resolve `to_host → dest-instance` via a `…/*/info` GET (§4.2).
3. Issue the `deliver` GET on `stylos/{realm}/stele/{dest-instance}/mailbox/deliver` with `DELIVER_TIMEOUT`. On `{ok:true, status:"stored"|"duplicate"}` → `mailbox_outbox_mark(DELIVERED, remote_message_id, delivered_at=now)`. On `{ok:false}` / timeout / no reply → `attempts += 1`, `last_error = reason|"timeout"`, `next_attempt_at = now + backoff(attempts)`.
4. After incrementing, if `attempts >= MAX_ATTEMPTS` **or** `now - created_at > OUTBOX_TTL` → `mailbox_outbox_mark(DEAD)` (dead-letter; visible via `outbox`).

**Alternative considered:** retry worker as a separate top-level tokio task with its own cancellation. Rejected: PRD-022 established `StylosHandle` as the single owner of mesh JoinHandles with one shutdown path; adding handles there keeps shutdown ordering in one place.

### 4.9 Reachability map (heartbeat listener)

The heartbeat is published today but unsubscribed (§3.1). This PRD adds a **heartbeat-listener task** that maintains an in-memory `host→last_seen` map, shared with the delivery worker.

- Type: `Arc<tokio::sync::Mutex<HashMap<String, std::time::Instant>>>` (key = raw `mailbox_host` string, value = last receipt instant), created in `start` and cloned into both the listener and the worker.
- The listener `declare_subscriber` on `stylos/{realm}/stele/*/info` is **not** used for liveness; instead it subscribes to `stylos/{realm}/stele/*/heartbeat` and, on each sample, GETs (or caches from a periodic `…/*/info` sweep) the sender instance's `mailbox_host` to key the map. **Simplification:** since the heartbeat key carries the `instance` segment and the worker already does a `…/*/info` GET per attempt to resolve `dest-instance`, the listener instead keys the map on **instance** and the worker checks freshness on the resolved `dest-instance`. The map is `instance→last_seen`; `to_host` reachability = "some instance whose `info.mailbox_host == to_host` has a fresh heartbeat".
- Owned by `StylosHandle` as `heartbeat_listener_task: JoinHandle<()>`, same `CancellationToken`, aborted+awaited in `shutdown`.
- A node never heartbeat-skips delivery to **itself** (handled at `send`: same-host stores directly, §4.5).

**Alternative considered:** drop the heartbeat-skip entirely and rely only on `DELIVER_TIMEOUT` burning attempts against dead hosts. Rejected: a permanently-offline `to_host` would dead-letter in `MAX_ATTEMPTS * DELIVER_TIMEOUT` ≈ 25s of wasted GETs and consume the attempt budget meant for transient failures; the freshness gate preserves the budget for real flakiness. (`DELIVER_TIMEOUT` is still kept as the upper bound for a host that *is* fresh but stalls mid-reply.)

### 4.10 Idempotency

The origin mints `mail_uid` = ULID at `send` time (via the existing `ulid` crate, `ulid::Ulid::new().to_string()`, same as `db.rs:683`). It travels on the wire in every `deliver` request and is the `UNIQUE` key at the destination. Result: at-least-once on the wire (retries are safe), exactly-once at rest (destination dedupes via `ON CONFLICT(mail_uid) DO NOTHING`), and a duplicate `deliver` returns the *existing* `message_id` with `status:"duplicate"` so the origin can finalize the outbox row regardless. The destination `message_id` (AUTOINCREMENT) is node-local and never used as a cross-node identifier.

### 4.11 Self-host determination & info blob

**One source of truth: the node's raw hostname, cached once at startup in `StylosStatusSource`.** Add a `mailbox_host: String` field to `StylosStatusSource` (`stylos_module.rs:20-27`), populated in `start` from `hostname::get()?.to_string_lossy().into_owned()` (the **raw, un-normalized** value, distinct from the normalized `instance`). It is:

1. Used by the `send` leaf to decide self-host: `to_host.eq_ignore_ascii_case(&status.mailbox_host)` → store locally; else spool.
2. Injected into the `info` queryable JSON blob (`stylos_module.rs:230-239`) as `"mailbox_host": status.mailbox_host` so other nodes can resolve `to_host → instance` (§4.2).

The `info` task already clones per-field locals (`q_instance`, `q_zid`, …); add `q_mailbox_host = status.mailbox_host.clone()` and include it in the `json!` blob.

### 4.12 `from` string composition & `mail_uid` minting

- **Minting:** at `send` time, on the **origin** node (inside the `send` queryable handler), `mail_uid = ulid::Ulid::new().to_string()`. It is written to both the outbox row (cross-node) and, for self-host sends, the inbox row.
- **`from` composition:** the **CLI** composes `from` before issuing `send`, from the resolved profile: `from = "{host}:{project_dir}"`, where `host`/`project_dir` come from `resolve_connection` + `CLAUDE_PROJECT_DIR`/`PWD` (same detection as `client.rs:69-75`). The optional `:alias` suffix is appended **only** when the user passes `--from-alias <a>` to `stele mail send` (a single sender role to stamp on the envelope, distinct from the recipient `--attention`). Absent → 2-segment `from`. The server trusts the CLI-supplied `from` verbatim (it is the local peer).

### 4.13 CLI: transient zenoh peer (stele-cli)

`stele mail *` lives behind a `mail` cargo feature in `stele-cli/Cargo.toml` that is **on by default** (`default = ["mail"]`) — a plain `cargo install`/`cargo build` ships the mail commands; `--no-default-features` yields the lean ureq-only build (e.g. for minimal/Docker CLI). The `mail` feature pulls `dep:stylos` + `dep:zenoh` + `dep:tokio` (mirroring the server's `stylos` feature composition; the same pinned `stylos` git tag v0.2.1 and `zenoh =1.9.0`).

`Cargo.toml` additions:

```toml
[features]
default = ["mail"]
mail = ["dep:stylos", "dep:zenoh", "dep:tokio"]

[dependencies]
# … existing …
stylos = { git = "https://github.com/tasanakorn/stylos.git", tag = "v0.2.1", optional = true }
zenoh  = { version = "=1.9.0", optional = true }
tokio  = { version = "1", features = ["rt-multi-thread", "macros", "time"], optional = true }
```

Because `main()` is sync (`main.rs:266`), the `Mail(..)` match arm builds its **own** runtime isolated from the sync command path: `let rt = tokio::runtime::Runtime::new()?; rt.block_on(commands::mail::run_mail(args, conn))`. The `Mail` subcommand variant, its match arm, the `commands::mail` module, and the `stylos_client` module are all `#[cfg(feature = "mail")]`-gated; with the feature off the base binary has no zenoh/tokio code. `run_mail`:

1. Opens a **transient** zenoh session via `stylos::open_session` with mode `client`, `connect: [local node endpoint]`, `listen` default-empty, `scouting: None` — explicit direct-connect, **no multicast scouting** (the documented direct-connect path, `docs/stylos/discovery.md:31-35`). **Mode `client`, not `peer`:** the CLI is a pure client of the local router node; a `peer` session lingers ~10s on `session.close()` (its session-close timeout), which dominated every invocation — `client` closes in <1ms. Identity: `realm` from profile/default `dev`, `role` `stele-cli`, `instance` `stele-cli`.
2. Reads the local node endpoint from a new `Profile.zenoh_endpoint` field (§4.14), default `tcp/127.0.0.1:31747`.
3. Resolves the **local** instance: GET `stylos/{realm}/stele/*/info`, pick the reply whose `mailbox_host` equals this machine's `gethostname` (the local node). Issue one GET against `stylos/{realm}/stele/{local-instance}/mailbox/{leaf}` with the leaf request JSON, print the JSON reply to stdout, map `ok` → exit code (§4.15), close the session, exit.

`StylosConfig` the CLI peer builds (concrete):

```rust
StylosConfig {
    stylos: IdentitySection { realm, role: "stele-cli".into(), instance: cli_instance },
    zenoh: ZenohSection {
        mode: "client".into(),
        connect: Endpoints { endpoints: vec![zenoh_endpoint] },
        listen: Endpoints::default(),
        scouting: None,
    },
}
// SessionOverrides { connect: Some(vec![zenoh_endpoint]) }
```

**Alternative considered:** add a long-lived `stele` daemon that holds a persistent peer. Rejected: the CLI is invoked per command; a transient direct-connect peer with no scouting keeps cold-start cost bounded and adds no new daemon (cold-start latency is an accepted risk, §6).

### 4.14 Config: `zenoh_endpoint`

Add to `Profile` (`config.rs:11-18`):

```rust
#[serde(default = "default_zenoh_endpoint")]
pub zenoh_endpoint: String,

fn default_zenoh_endpoint() -> String { "tcp/127.0.0.1:31747".to_string() }
```

`Default for SteleConfig` (`config.rs:20-36`) seeds the `local` profile with `zenoh_endpoint: default_zenoh_endpoint()`. `resolve_connection` (`config.rs:144-175`) gains a fourth return element: change its signature to also return the resolved `zenoh_endpoint` (CLI flag `--zenoh-endpoint` > env `STELE_ZENOH_ENDPOINT` > profile field > default). The `Mail` handler reads it; the existing sync callers ignore the extra tuple element (update the destructuring at `main.rs:304`).

### 4.15 CLI command spec

`stele mail <sub>` (all `#[cfg(feature = "mail")]`). Global `--zenoh-endpoint` / `--profile` resolve the peer endpoint (§4.14). Stdout is the **raw JSON reply** (scriptable; matches that existing commands honor `--json`, but the mail surface is JSON-only since replies are already JSON). Exit codes: `0` success (`ok:true`), `3` not-found (`error:"not_found"`), `4` undeliverable/conflict (`error:"undeliverable"|"conflict"`), `2` bad request (`error:"bad_request"`), `1` transport/internal.

| Subcommand | Flags / args                                                                                                                  | Leaf      | stdout                                  |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------- | --------- | --------------------------------------- |
| `send`     | `--to-host <h>` (req), `--to-project <p>` (req), `--attention <a>` (opt), `--subject <s>` (opt), `--type <t>` (opt, def `NOTE`), `--meta <json>` (opt), `--payload <json>` (opt), `--from-alias <a>` (opt) | `send`    | `{ok,mail_uid,message_id?,status}`      |
| `list`     | `--alias <a>` (opt, repeatable), `--status <s>` (opt, repeatable, def `NEW,READ`), `--project <p>` (opt, def detected pwd)     | `list`    | `{ok,messages:[…]}`                     |
| `read`     | `<message_id>` (positional, req)                                                                                              | `read`    | `{ok,message_id,status}`                |
| `archive`  | `<message_id>` (positional, req)                                                                                              | `archive` | `{ok,message_id,status}`                |
| `get`      | `<message_id>` (positional, req)                                                                                              | `get`     | `{ok,message:{…}}`                      |
| `outbox`   | `--status <s>` (opt, repeatable, def `QUEUED,DEAD`)                                                                           | `outbox`  | `{ok,rows:[…]}`                         |
| `register` | `--alias <a>` (req, repeatable), `--project <p>` (opt, def detected pwd)                                                      | `list`*   | `{ok,registered:[…]}`                   |

`--project` defaults to the detected project dir (`CLAUDE_PROJECT_DIR` > `PWD`, sanitized like `client.rs:69-75`). `--meta`/`--payload` accept a JSON string parsed client-side (bad JSON → exit `2` before any network call). `*register` rides the `list` leaf with `register_only:true`.

### 4.16 Encoding and reserved values

JSON on the mesh throughout (`Encoding::APPLICATION_JSON`). `attention` match is case-insensitive (aliases stored lowercased; comparisons lowercased). `*` is reserved for household-broadcast and is rejected as an alias on registration (`mailbox_alias_upsert` skips/`bad_request`-rejects a `*` alias).

## 5. Changes by Component

| Component | Symbols to add / modify | Files |
| --------- | ----------------------- | ----- |
| stele-cli Cargo manifest | Add `[features] default = ["mail"]` + `mail = [...]`; make `stylos`/`zenoh`/`tokio` optional deps. | `apps/stele/crates/stele-cli/Cargo.toml` |
| stele-cli config | Add `Profile.zenoh_endpoint` + `default_zenoh_endpoint`; seed in `Default`; widen `resolve_connection` return to include endpoint; add `--zenoh-endpoint`/`STELE_ZENOH_ENDPOINT` resolution. | `apps/stele/crates/stele-cli/src/config.rs` |
| stele-cli commands | Add `#[cfg(feature="mail")]` `Mail { Send/List/Read/Archive/Get/Outbox/Register }` clap subcommand + variants; dispatch arm builds `tokio::runtime::Runtime` and `block_on`s `commands::mail::run_mail`; update `resolve_connection` destructure at `:304`. | `apps/stele/crates/stele-cli/src/main.rs`, `apps/stele/crates/stele-cli/src/commands/mail.rs` (new), `commands/mod.rs` |
| stele-cli transient peer | New `stylos_client.rs`: `fn open_local_peer(endpoint, realm) -> Session`, `fn resolve_local_instance(&Session, realm) -> String`, `fn query_leaf(&Session, key, req_json) -> Value`; request/reply structs mirroring §4.5. | `apps/stele/crates/stele-cli/src/stylos_client.rs` (new) |
| stele-server mesh leaves + worker | `serve_mailbox_queryables` (declares 7 queryables, one task), `spawn_delivery_worker`, `spawn_heartbeat_listener`; add `delivery_task` + `heartbeat_listener_task` to `StylosHandle` (+ abort/await in `shutdown`); add `mailbox_host` to `StylosStatusSource` + `info` blob; `DeliverReq`/`DeliverReply` + per-leaf req/reply structs; consts table (§4.8). | `apps/stele/crates/stele-server/src/stylos_module.rs` |
| stele-server storage | Add `mailbox_inbox`/`mailbox_outbox`/`mailbox_agent_alias` + indexes to the main schema block (`db.rs:141` batch); add `DROP TABLE IF EXISTS steop_mailbox` to `ensure_steop_schema`; rename `SteopMailboxRow`→`MailboxRow` (+`mail_uid`,`attention`,`from_addr`); add `mailbox_inbox_send/_get/_list/_read/_archive`, `mailbox_outbox_enqueue/_due/_mark/_list`, `mailbox_alias_upsert/_list`; keep `MailboxTransition`. | `apps/stele/crates/stele-server/src/db.rs` |
| stele-server REST mailbox types | Add optional `attention` to `MailboxSendReq`/`MailboxListReq` (`steop_api.rs:127-154`); thread into the (still REST) `steop_mailbox_send` call as a passthrough; existing 2-seg id parsing unchanged. **Note:** with `steop_mailbox` reshaped to `mailbox_inbox`, the REST handlers must call the new `mailbox_inbox_*` fns; `to_id`→`to_project` mapping derived from `parse_id(req.to).project_dir`. | `apps/stele/crates/stele-server/src/steop_api.rs` |
| Docs — addressing | Document `mailbox/*` leaves, `mailbox_host` claim, host→instance resolution. | `docs/stylos/addressing.md` |
| Docs — steop design | Note attention envelope replaces `:UUID`/`:USER` 3rd segment for postal mail; Go REST path unchanged. | `docs/steop/DESIGN.md` |
| Docs — HTTP API | Document `mail` surface is on zenoh not REST; note `attention` on retained REST types. | `docs/stele/http-api.md` |
| Docs — PRD index | Add PRD-027 row after PRD-026. | `docs/README.md` |
| Version bump | Lock-step `0.19.2 → 0.20.0`. | `apps/stele/Cargo.toml`, `apps/steop/version.go`, `plugins/stele/.claude-plugin/plugin.json`, `plugins/steop/.claude-plugin/plugin.json` |

No changes to `apps/stylos/**` (external, pinned v0.2.1) or `apps/steop/internal/client/mailbox.go` (legacy Go client, left untouched).

## 6. Edge Cases

| Scenario | Behavior |
| -------- | -------- |
| `to_host` is an illegal-instance hostname (e.g. `MacBook.local`) | No host normalization imposed. Destination publishes its own `mailbox_host` claim in `info`; origin matches `to_host` (case-insensitive) against the claim. No two-namespace collision. |
| No live node claims `to_host` | Outbox row stays `QUEUED`. The worker **skips** it (no fresh heartbeat, §4.9) without burning an attempt, until a claiming node appears or `OUTBOX_TTL` expires. |
| Destination offline mid-flight, comes back later | Origin retries with backoff; on success marks `DELIVERED`. Destination dedupes on `mail_uid` — no duplicate inbox row even after a half-succeeded attempt. |
| Duplicate `deliver` (retry after a lost reply) | Destination finds existing `mail_uid`, returns `{ok:true, message_id, status:"duplicate"}`. Origin finalizes the outbox row `DELIVERED`. |
| Mail to a home/project never seen before | Destination auto-creates the inbox row on first `deliver` (no homes table; first row is the home). No pre-registration. |
| `attention = '*'` on a `list` | Visible to every caller (household broadcast). `*` rejected as an alias on upsert so it can never collide with a real recipient. |
| `attention = null` | Household mail — visible to every reader of the home regardless of alias set. |
| Two agents on one home share an alias | Both see the message (set membership, not exclusive). Intended: aliases are roles. |
| CLI cold-start latency per invocation | Each invocation opens a transient peer (direct connect, no scouting) then exits. Accepted cost; mitigated by skipping scouting. Measure in testing. |
| Shutdown with an in-flight `deliver` | Worker shares the `CancellationToken`; an aborted in-flight delivery may leave the outbox row `QUEUED`. Next boot retries; destination dedupe on `mail_uid` prevents a double inbox row. |
| Dead-letter reached (≥ `MAX_ATTEMPTS` or > `OUTBOX_TTL`) | Outbox row marked `DEAD`; no further attempts. Surfaced via `stele mail outbox`; operator requeues or drops. |
| Old REST caller (steop Go client) sends with no `attention` | `attention` optional on retained REST types; null → household mail. Go REST path otherwise unchanged. |
| `--meta`/`--payload` not valid JSON | CLI exits `2` (`bad_request`) before opening a session. |

## 7. Migration

- **Additive new surface, reshaped inbox table.** `mailbox_inbox`/`mailbox_outbox`/`mailbox_agent_alias` are created in the main schema block. `steop_mailbox` is dropped in `ensure_steop_schema` and superseded by `mailbox_inbox` (gaining `mail_uid` + `attention`). Because PRD-001 §9 / `ensure_steop_schema` already declares mailbox data **not preserved** across schema changes, existing rows are **not** migrated — a clean cutover consistent with prior mailbox reshapes.
- **Identity grammar unchanged for the home id.** The 2-segment `host:project_dir` home id stays valid under both existing parsers (`steop_api.rs:53-95`, `identity.go`). `attention` is a new envelope field outside the id, so no parser change is required and the closed-set 3rd-segment rule is simply not exercised by postal mail.
- **steop Go client untouched.** `apps/steop/internal/client/mailbox.go` keeps its REST path. The retained `steop.mailbox.*` REST types gain an optional `attention` field (backward-compatible — absent = household).
- **Lock-step version bump** via `python scripts/bump-version.py 0.20.0` — moves the workspace + both plugins + `steop/version.go` in one commit. No stylos bump (external, pinned v0.2.1). CI validates `plugin.json` == Cargo version.
- **Default mesh participation.** Every stele-server already joins the mesh (PRD-022 default-on). On upgrade, nodes begin serving `mailbox/*` leaves, publishing `mailbox_host` in `info`, and running the delivery worker + heartbeat listener automatically; no operator config required.

## 8. Testing

No automated harness; manual smoke matches the rest of the workspace. The single-host steps below are codified in `apps/stele/scripts/smoke-mail.py` (stdlib-only, drives `stele mail` via subprocess, asserts, archives on cleanup — mirrors `apps/steop/scripts/smoke-mailbox.py`); cross-node steps run with `PEER_HOST=<host>` set.

1. **Build both feature shapes** → verify: `cargo build -p stele-cli` (default — includes `mail`) and `cargo build -p stele-cli --no-default-features` (lean, ureq-only) both compile clean; `cargo build -p stele-server` carries the new queryables + worker.
2. **Schema creation** → verify: start stele-server, `sqlite3 stele.db ".tables"` lists `mailbox_inbox`, `mailbox_outbox`, `mailbox_agent_alias`; `sqlite3 stele.db "SELECT name FROM sqlite_master WHERE name='steop_mailbox'"` is empty.
3. **`mailbox_host` in info** → verify: `cargo run -p stylos-cli -- --connect tcp/127.0.0.1:31747 get 'stylos/dev/stele/*/info'` returns a JSON blob containing `"mailbox_host"` equal to the node's `gethostname`.
4. **Local send (same host)** → verify: `stele mail send --to-host "$(hostname)" --to-project /p --subject hi` returns `status:"delivered"` with a `mail_uid`; `stele mail list --project /p` shows the row `status=NEW`; `sqlite3 stele.db "SELECT mail_uid,status FROM mailbox_inbox WHERE to_project='/p'"` shows it.
5. **Cross-node deliver (two hosts)** → verify: on A `stele mail send --to-host <B-host> --to-project /p --subject hi`; on B `stele mail list --project /p` shows the row; on A `stele mail outbox` shows the row `DELIVERED` with a non-null `remote_message_id`.
6. **Offline destination spool** → verify: `stele mail send --to-host <B-host> …` while B is stopped; A's `stele mail outbox` shows `QUEUED attempts=0` (skipped, no heartbeat); start B; within one `WORKER_TICK` (5s) the row flips `DELIVERED` and appears in B's `list`.
7. **Idempotency** → verify: force a duplicate `deliver` (kill A after B replies, restart A); `sqlite3 B/stele.db "SELECT COUNT(*) FROM mailbox_inbox WHERE mail_uid='<uid>'"` returns `1`; A's outbox row is `DELIVERED` with the same `remote_message_id`.
8. **Attention routing** → verify: `stele mail register --alias claude --project /p`; send three messages with `--attention claude`, no attention, and `--attention '*'`; `stele mail list --alias claude --project /p` returns all three; `stele mail list --alias other --project /p` returns only the null + `*` rows, hiding `claude`.
9. **Dead-letter** → verify: send to a host that claims `to_host` but whose `deliver` always errors (or build with `MAX_ATTEMPTS=2`); after the cap `stele mail outbox --status DEAD` shows the row `DEAD` with `last_error` set and no further `attempts` growth.
10. **REST backward-compat** → verify: `curl -XPOST localhost:3100/api/v1/steop/mailbox.send -d '{"id":"h:/p","to":"h:/p"}'` returns 200 and stores a household (null-attention) row; `sqlite3 stele.db "SELECT attention FROM mailbox_inbox ORDER BY message_id DESC LIMIT 1"` is NULL.
11. **Shutdown cleanliness** → verify: Ctrl-C / Quit stele-server with a `QUEUED` outbox row; no panic; on restart the worker resumes and delivers without a duplicate inbox row at the destination.
