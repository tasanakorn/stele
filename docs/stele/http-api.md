# HTTP API Reference

## Overview

Stele exposes a JSON API at `/api/v1`. The stele memory and graph endpoints are REST; the steop workflow endpoints are RPC (see [Steop Extensions](#steop-extensions)). CORS is fully permissive (all origins allowed). All error responses use the format:

```json
{ "error": "message" }
```

---

## Authentication

By default, stele-server is open with no authentication required. When a pre-shared key is configured (via `--auth-key`, `STELE_AUTH_KEY`, or `auth_key` in `config.toml`), all HTTP and MCP routes require clients to send the key in the `X-Stele-Key` request header.

- **Header:** `X-Stele-Key: <key>`
- **When auth is disabled:** all routes are open (default behavior unchanged).
- **When auth is enabled:** missing or incorrect key returns `401 Unauthorized`:

```json
{ "error": "unauthorized" }
```

CORS preflight (`OPTIONS`) requests are not blocked — the middleware only enforces the header on actual requests.

The stele CLI sends `X-Stele-Key` automatically when a key is configured in its profile.

---

## Health

### GET /api/v1/health

Liveness probe. Reports overall server status, version, DB connectivity, and (optionally) the stylos/zenoh session summary. Unauthenticated — same posture as `/api/v1/stats`.

**Response (stylos feature on, session active):**

```json
{
  "status": "ok",
  "version": "0.17.0",
  "db_ok": true,
  "stylos": {
    "enabled": true,
    "mode": "router",
    "zid": "e0a1c9...",
    "realm": "dev",
    "instance": "dev-mbp",
    "listen_endpoints": [],
    "peers": 2,
    "routers": 0
  }
}
```

`listen_endpoints` is currently always `[]` — zenoh 1.9 exposes no stable public listener enumeration API (see [PRD-022](../prd/prd-022-stylos-in-stele-server.md) known limitations).

**Response (stylos feature on, `[stylos].enabled = false`):**

```json
{
  "status": "ok",
  "version": "0.17.0",
  "db_ok": true,
  "stylos": { "enabled": false }
}
```

**Response (stylos feature off, e.g. `headless-minimal` build):**

```json
{
  "status": "ok",
  "version": "0.17.0",
  "db_ok": true
}
```

The `stylos` key is omitted (not `null`) when the cargo feature is off so the JSON shape matches the binary's actual capability.

---

## Memory Endpoints

### POST /api/v1/memories

Create a new memory.

**Request body:**

```json
{
  "title":       "string (required)",
  "content":     "string (required)",
  "scope":       "string (required)",
  "tags":        ["string"] ,
  "memory_type": "string (optional)",
  "author":      "string (optional)"
}
```

`memory_type` values: `knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other`. Defaults to `knowledge`.

**Response:** 201 Created + Memory object

**Error:** 500 + error object

---

### GET /api/v1/memories

Search or list memories.

**Query parameters:**

| Parameter      | Type   | Description                                                                     |
| -------------- | ------ | ------------------------------------------------------------------------------- |
| q              | string | Full-text search query                                                          |
| scope          | string | Comma-separated scope prefixes (prefix-matched)                                 |
| tags           | string | Comma-separated tag filter                                                      |
| match_all_tags | bool   | If `true`, memory must have ALL specified tags. Default: `false`                |
| limit          | number | Max results (default: 20, max: 100)                                             |

When `q` is provided, results are ordered by FTS rank (relevance). Without `q`, results are ordered by `updated_at` DESC.

**Response:** 200 + SearchResult[]

---

### GET /api/v1/memories/{id}

Get a single memory by ID.

**Response:** 200 + Memory object, or 404

---

### PUT /api/v1/memories/{id}

Update a memory. Only provided fields are changed. `author` cannot be updated after creation.

**Request body:**

```json
{
  "title":       "string (optional)",
  "content":     "string (optional)",
  "scope":       "string (optional)",
  "tags":        ["string (optional — replaces all existing tags)"],
  "memory_type": "string (optional)"
}
```

**Response:** 200 + updated Memory object, or 404

---

### DELETE /api/v1/memories/{id}

Delete a memory.

**Response:** 200 + `{"deleted": true, "id": "..."}`, or 404

---

### GET /api/v1/scopes

List all scopes with memory counts.

**Query parameters:**

| Parameter | Type   | Description                      |
| --------- | ------ | -------------------------------- |
| prefix    | string | Optional prefix filter           |

**Response:** 200 + ScopeInfo[]

---

### GET /api/v1/tags

List all tags with memory counts.

**Query parameters:**

| Parameter | Type   | Description                                               |
| --------- | ------ | --------------------------------------------------------- |
| scope     | string | Comma-separated scope prefixes (prefix-matched)           |

**Response:** 200 + TagInfo[], ordered by count DESC

---

### GET /api/v1/stats

Dashboard summary statistics.

**Response:** 200

```json
{
  "total_memories":  1234,
  "total_scopes":    42,
  "total_tags":      88,
  "recent_memories": [
    { "id": "...", "title": "...", "scope": "...", "updated_at": "..." }
  ]
}
```

`recent_memories` contains the 5 most recently updated memories.

---

## Knowledge Graph Endpoints

### GET /api/v1/graph

Read the full knowledge graph for one or more scopes.

**Query parameters:**

| Parameter | Type   | Description                                               |
| --------- | ------ | --------------------------------------------------------- |
| scope     | string | Comma-separated scope prefixes (prefix-matched)           |

**Response:** 200 + Graph `{entities, relations}`

---

### POST /api/v1/graph/entities

Create entities. Idempotent — existing entities get observations appended.

**Request body:**

```json
{
  "entities": [
    { "name": "...", "entity_type": "...", "observations": ["..."] }
  ],
  "scope": "..."
}
```

**Response:** 201 + `[{name, id, created}]`

---

### GET /api/v1/graph/entities

Search or list entities.

**Query parameters:**

| Parameter | Type   | Description                                                            |
| --------- | ------ | ---------------------------------------------------------------------- |
| q         | string | Full-text search query                                                 |
| scope     | string | Comma-separated scope prefixes                                         |
| limit     | number | Max results (default: 20, max: 100)                                    |

With `q`: returns EntitySearchResult[] ranked by relevance. Without `q`: returns Entity[] for the given scope.

**Response:** 200

---

### GET /api/v1/graph/entities/{name}

Get an entity by name.

**Query parameters:**

| Parameter | Type   | Description                                   |
| --------- | ------ | --------------------------------------------- |
| scope     | string | Exact scope match (default: empty string)     |

**Response:** 200 + Entity object, or 404

---

### DELETE /api/v1/graph/entities/{name}

Delete an entity. Cascades to its observations and any relations where it is the source or target.

**Query parameters:**

| Parameter | Type   | Description                  |
| --------- | ------ | ---------------------------- |
| scope     | string | Scope of the entity          |

**Response:** 200 + `{"deleted": true, "id": "..."}`, or 404

---

### POST /api/v1/graph/entities/{name}/observations

Add observations to an existing entity.

**Query parameters:**

| Parameter | Type   | Description          |
| --------- | ------ | -------------------- |
| scope     | string | Scope of the entity  |

**Request body:**

```json
{ "observations": ["fact one", "fact two"] }
```

**Response:** 200 + updated Entity object, or 404

---

### DELETE /api/v1/graph/entities/{name}/observations

Delete specific observations from an entity by exact content match.

**Query parameters:**

| Parameter | Type   | Description          |
| --------- | ------ | -------------------- |
| scope     | string | Scope of the entity  |

**Request body:**

```json
{ "observations": ["exact fact to remove"] }
```

**Response:** 200 + updated Entity object, or 404

---

### POST /api/v1/graph/relations

Create relations. Idempotent. Both entities must exist in the given scope.

**Request body:**

```json
{
  "relations": [
    { "from": "EntityA", "to": "EntityB", "relation_type": "depends_on" }
  ],
  "scope": "..."
}
```

**Response:** 201 + `[{from, to, relation_type, id, created}]`, or 404 if an entity is not found

---

### DELETE /api/v1/graph/relations

Delete specific relations.

**Request body:**

```json
{
  "relations": [
    { "from": "EntityA", "to": "EntityB", "relation_type": "depends_on" }
  ],
  "scope": "..."
}
```

**Response:** 200 + `{"deleted": [...], "not_found": [...]}`

---

### GET /api/v1/graph/open

Open specific entities by name with their direct neighbor relations.

**Query parameters:**

| Parameter | Type   | Description                                               |
| --------- | ------ | --------------------------------------------------------- |
| names     | string | Comma-separated entity names (required)                   |
| scope     | string | Comma-separated scope prefixes                            |

**Response:** 200 + Graph containing the named entities and their neighbors

---

## Steop Extensions

The stele server hosts the cross-agent RPC surface used by the **steop** workflow pipeline at `/api/v1/steop/*`. These endpoints live in the same process but are served by a separate router (`apps/stele/crates/stele-server/src/steop_api.rs`) and back onto the `steop_mailbox` table. Design rationale and schema details are in [`../steop/DESIGN.md`](../steop/DESIGN.md).

> The session, state, storage, and log surfaces moved to a steop-local SQLite at v0.16.0 per [PRD-020](../prd/prd-020-steop-local-backend.md). Only `mailbox` and `notify` remain on stele-server as the cross-agent surface.

### Transport shape

Every steop method is:

```
POST /api/v1/steop/<method>
Content-Type: application/json
X-Stele-Key: <auth key>

{ ...body... }
```

There are no path parameters, no query parameters, and no header-based identity. The method name is dot-separated (`steop.mailbox.send`, `steop.notify`, ...). Responses are JSON objects; errors use `{ "error": "message" }` with an appropriate HTTP status code.

### Composite identity

Steop uses SSH/SCP-style composite identifiers in the request body:

- Project: `host:project_dir` — 2-segment
- Session: `host:project_dir:UUID` — 3-segment (canonical 8-4-4-4-12 UUID)
- User: `host:project_dir:USER` — 3-segment (literal `USER`)

The server validates segment grammar but not semantic correctness (hostname format, absolute path, etc.). Clients are responsible for identifier completeness.

### Method catalogue

#### Mailbox

| Method                      | Body                                                      | Response                   |
| --------------------------- | --------------------------------------------------------- | -------------------------- |
| `steop.mailbox.send`        | `{id, to, from?, subject?, message_type?, meta?, payload?}` | `MailboxRow`             |
| `steop.mailbox.list`        | `{id, to?, status?=["NEW"], message_type?, limit?=200}`   | `{messages: MailboxRow[]}` |
| `steop.mailbox.get`         | `{id, message_id}`                                        | `MailboxRow`               |
| `steop.mailbox.read`        | `{id, message_id}`                                        | `{message_id, status:"READ"}`     |
| `steop.mailbox.archive`     | `{id, message_id}`                                        | `{message_id, status:"ARCHIVE"}`  |
| `steop.mailbox.update_meta` | `{id, message_id, meta_patch}`                            | `MailboxRow`               |

Sender may be any principal (project, session, or user). Recipient may be any principal. The server derives `from` from the caller's `id` when `from` is omitted — explicit `from` overrides. Ordered `created_at` ASC (FIFO). Status lifecycle is `NEW -> READ -> ARCHIVE`: `mailbox.read` transitions `NEW -> READ`, `mailbox.archive` transitions `NEW -> ARCHIVE` or `READ -> ARCHIVE`. Illegal transitions return 409. `mailbox.get` is side-effect-free. Default `list` filter is `status:["NEW"]`; pass an explicit array to widen. `mailbox.update_meta` shallow-merges the supplied `meta_patch` object into the target row's `meta` column (keys in the patch overwrite top-level keys in the existing meta, others preserved, nested objects replaced wholesale) and returns the updated row; does not touch `status`, so legal on any row regardless of lifecycle state. Returns 404 on unknown `message_id`. See `docs/prd/prd-001-mailbox-v2.md` for the normative spec and `docs/prd/prd-014-mailbox-watch-flag-parsing.md` for the `update_meta` design.

#### Notifications

| Method         | Body                                            | Response    |
| -------------- | ----------------------------------------------- | ----------- |
| `steop.notify` | `{title?, body?, subtitle?, sound?=false}`      | `{}` or 501 |

Fire-and-forget local OS notification. Desktop builds render via system notification; headless builds return `501 Not Implemented`.

### Steop response types

```json
// MailboxRow
{
  "message_id":   1234,
  "from":         "string (composite id: HOST:PROJECT_DIR[:SESSION_UUID|:USER])",
  "to":           "string (composite id)",
  "subject":      "string",
  "message_type": "string (HOOK:* | TASK:* | NOTE:* | CHAT:MESSAGE)",
  "meta":         {},
  "payload":      {},
  "created_at":   "string (RFC3339)",
  "status":       "NEW | READ | ARCHIVE"
}
```

---

## Type Definitions

### Memory

```json
{
  "id":          "string (ULID)",
  "title":       "string",
  "content":     "string",
  "memory_type": "knowledge | decision | convention | troubleshooting | reference | other",
  "scope":       "string",
  "author":      "string | null",
  "tags":        ["string"],
  "created_at":  "string (RFC3339)",
  "updated_at":  "string (RFC3339)"
}
```

### SearchResult

Same fields as Memory, with one additional field:

```json
{
  "rank": "number | null"
}
```

`rank` is an FTS relevance score. Present and non-null when the result was returned by a full-text search query; `null` when results are returned without a query.

### Entity

```json
{
  "id":           "string (ULID)",
  "name":         "string",
  "entity_type":  "string",
  "scope":        "string",
  "observations": ["Observation"],
  "created_at":   "string (RFC3339)",
  "updated_at":   "string (RFC3339)"
}
```

### Observation

```json
{
  "id":         "string (ULID)",
  "content":    "string",
  "created_at": "string (RFC3339)"
}
```

### Relation

```json
{
  "id":             "string (ULID)",
  "from_entity":    "string (entity name)",
  "from_entity_id": "string (entity ULID)",
  "to_entity":      "string (entity name)",
  "to_entity_id":   "string (entity ULID)",
  "relation_type":  "string",
  "scope":          "string",
  "created_at":     "string (RFC3339)"
}
```

### Graph

```json
{
  "entities":  ["Entity"],
  "relations": ["Relation"]
}
```

### ScopeInfo

```json
{
  "scope": "string",
  "count": "number"
}
```

### TagInfo

```json
{
  "tag":   "string",
  "count": "number"
}
```

### EntitySearchResult

```json
{
  "entity": "Entity",
  "rank":   "number | null"
}
```

---

## Differences from MCP

The REST API and MCP tools expose the same underlying data but differ in how multi-value parameters are passed.

| Aspect                 | REST API                          | MCP Tools                       |
| ---------------------- | --------------------------------- | ------------------------------- |
| Scope (multi)          | Comma-separated string            | String or JSON array            |
| Tags (multi)           | Comma-separated string            | JSON array or string-encoded    |
| Entity observations    | JSON array                        | JSON array or string-encoded    |
| Scope on entity get    | Exact match                       | Exact match                     |
| Scope on search        | Prefix match                      | Prefix match                    |
