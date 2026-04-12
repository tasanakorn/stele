# HTTP API Reference

## Overview

Stele exposes a JSON API at `/api/v1`. The stele memory and graph endpoints are REST; the steop workflow endpoints are RPC (see [Steop Extensions](#steop-extensions)). CORS is fully permissive (all origins allowed). All error responses use the format:

```json
{ "error": "message" }
```

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

The stele server hosts the RPC surface used by the **steop** workflow pipeline at `/api/v1/steop/*`. These endpoints live in the same process but are served by a separate router (`apps/stele/crates/stele-server/src/steop_api.rs`) and back onto the `steop_sessions`, `steop_storage_session`, `steop_storage_project`, `steop_mailbox`, and `steop_logs` tables. Design rationale and schema details are in [`../steop/DESIGN.md`](../steop/DESIGN.md).

### Transport shape

Every steop method is:

```
POST /api/v1/steop/<method>
Content-Type: application/json
X-Stele-Key: <auth key>

{ ...body... }
```

There are no path parameters, no query parameters, and no header-based identity. The method name is dot-separated (`steop.session.start`, `steop.storage.put`, ...). Responses are JSON objects; errors use `{ "error": "message" }` with an appropriate HTTP status code.

### Composite identity

Steop uses SSH/SCP-style composite identifiers in the request body:

- Project: `host` + `project_dir` (e.g. `vm-02` + `/home/tas/stele`)
- Session: `host` + `project_dir` + `session_id`

`session_id` is a globally unique Claude Code UUID. Read methods (`session.get`, `state.get`, `status.get`) accept a bare `{session_id}` as a short form; write methods require the full triple. The server performs no validation — clients are responsible for identifier completeness.

The v0.5 `X-Steop-Host` / `X-Steop-Project-Dir` headers are **ignored** by v2.

### Method catalogue

#### Session lifecycle

| Method                | Body                                                           | Response                              |
| --------------------- | -------------------------------------------------------------- | ------------------------------------- |
| `steop.session.start` | `{host, project_dir, session_id, data?}`                       | `Session`                             |
| `steop.session.stop`  | `{host, project_dir, session_id}`                              | `Session`                             |
| `steop.session.touch` | `{host, project_dir, session_id}`                              | `Session`                             |
| `steop.session.get`   | `{session_id}` or `{host, project_dir, session_id}`            | `Session` or 404                      |
| `steop.session.list`  | `{host?, project_dir?, state?, limit?}`                        | `{sessions: Session[]}`               |
| `steop.project.list`  | `{host?}`                                                      | `{projects: [{host, project_dir}]}`   |

`start` is idempotent: an existing row is reactivated (`state='active'`, `stopped_at=null`, `data` merged). `list` is ordered by `last_active_at` DESC, default `limit=100`. `state` filter accepts `"active"` or `"stopped"`.

#### State and counters

| Method               | Body                                                         | Response                  |
| -------------------- | ------------------------------------------------------------ | ------------------------- |
| `steop.state.get`    | `{session_id}` or full triple                                | `Session` or 404          |
| `steop.state.put`    | `{host, project_dir, session_id, data, merge?=true}`         | `Session`                 |
| `steop.state.incr`   | `{host, project_dir, session_id, counter, delta?=1}`         | `{counter, value}`        |
| `steop.state.reset`  | `{host, project_dir, session_id, counter, value?=0}`         | `{counter, value}`        |
| `steop.state.delete` | `{host, project_dir, session_id}`                            | `{deleted: true\|false}`  |

`state.put` performs a shallow merge of top-level keys unless `merge:false` replaces `data` entirely. `incr`/`reset` operate on the `counters` JSON column. All writes refresh `last_active_at` and create the session row if absent.

#### Statusline

| Method             | Body                          | Response                         |
| ------------------ | ----------------------------- | -------------------------------- |
| `steop.status.get` | `{session_id}` or full triple | `StatusProjection` (always 200)  |

Projects `{session_id, mode, phase, step, tool_calls, loop_count, step_retry, last_active_at}`. Unknown sessions return defaulted values.

#### Storage

| Method                 | Body                                             | Response                              |
| ---------------------- | ------------------------------------------------ | ------------------------------------- |
| `steop.storage.put`    | `{host, project_dir, key, content, session_id?}` | `{key, updated_at}`                   |
| `steop.storage.get`    | `{host, project_dir, key, session_id?}`          | `StorageBlob` or 404                  |
| `steop.storage.delete` | `{host, project_dir, key, session_id?}`          | `{deleted: true\|false}`              |
| `steop.storage.list`   | `{host, project_dir, session_id?}`               | `{items: [{key, updated_at, size}]}`  |

Presence of `session_id` selects `steop_storage_session`; absence selects `steop_storage_project`.

#### Log

| Method             | Body                                            | Response            |
| ------------------ | ----------------------------------------------- | ------------------- |
| `steop.log.append` | `{host, project_dir, session_id, event, data?}` | `{id}`              |
| `steop.log.query`  | `{host?, project_dir?, session_id?, limit?=200}`| `{logs: LogRow[]}`  |

Ordered `created_at` DESC. All filter fields are optional and combine additively.

#### Mailbox

| Method                  | Body                                                      | Response                   |
| ----------------------- | --------------------------------------------------------- | -------------------------- |
| `steop.mailbox.send`    | `{id, to, from?, subject?, message_type?, meta?, payload?}` | `MailboxRow`             |
| `steop.mailbox.list`    | `{id, to?, status?=["NEW"], message_type?, limit?=200}`   | `{messages: MailboxRow[]}` |
| `steop.mailbox.get`     | `{id, message_id}`                                        | `MailboxRow`               |
| `steop.mailbox.read`    | `{id, message_id}`                                        | `{message_id, status:"READ"}`     |
| `steop.mailbox.archive` | `{id, message_id}`                                        | `{message_id, status:"ARCHIVE"}`  |

Sender may be any principal (project, session, or user). Recipient may be any principal. The server derives `from` from the caller's `id` when `from` is omitted — explicit `from` overrides. Ordered `created_at` ASC (FIFO). Status lifecycle is `NEW -> READ -> ARCHIVE`: `mailbox.read` transitions `NEW -> READ`, `mailbox.archive` transitions `NEW -> ARCHIVE` or `READ -> ARCHIVE`. Illegal transitions return 409. `mailbox.get` is side-effect-free. Default `list` filter is `status:["NEW"]`; pass an explicit array to widen. See `docs/prd/prd-001-mailbox-v2.md` for the normative spec.

#### Notifications

| Method         | Body                                            | Response    |
| -------------- | ----------------------------------------------- | ----------- |
| `steop.notify` | `{title?, body?, subtitle?, sound?=false}`      | `{}` or 501 |

Fire-and-forget local OS notification. Desktop builds render via system notification; headless builds return `501 Not Implemented`.

### Steop response types

```json
// Session
{
  "host":           "string",
  "project_dir":    "string",
  "session_id":     "string",
  "state":          "active | stopped",
  "started_at":     "string (RFC3339)",
  "last_active_at": "string (RFC3339)",
  "stopped_at":     "string (RFC3339) | null",
  "data":           {},
  "counters":       { "tool_calls": 12 }
}

// StatusProjection
{
  "session_id":     "string",
  "mode":           "string",
  "phase":          "string",
  "step":           "string",
  "tool_calls":     0,
  "loop_count":     0,
  "step_retry":     0,
  "last_active_at": "string (RFC3339)"
}

// StorageBlob
{
  "host":        "string",
  "project_dir": "string",
  "session_id":  "string | null",
  "key":         "string",
  "content":     "string",
  "created_at":  "string (RFC3339)",
  "updated_at":  "string (RFC3339)"
}

// LogRow
{
  "id":          1234,
  "host":        "string",
  "project_dir": "string",
  "session_id":  "string",
  "event":       "string",
  "data":        {},
  "created_at":  "string (RFC3339)"
}

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

### Migrated from v0.5

Every REST route that previously used path or query parameters has been removed. Clients on v0.5 must migrate to the RPC methods above — there is no deprecation window (stele is pre-1.0). A rough mapping:

| v0.5 route                                | v0.6 method            |
| ----------------------------------------- | ---------------------- |
| `PUT /steop/storage?scope=&key=`          | `steop.storage.put`    |
| `GET /steop/storage?scope=&key=`          | `steop.storage.get`    |
| `DELETE /steop/storage?scope=&key=`       | `steop.storage.delete` |
| `GET /steop/storage/list?scope=`          | `steop.storage.list`   |
| `GET /steop/storage/scopes`               | `steop.project.list`   |
| `GET /steop/state/:id`                    | `steop.state.get`      |
| `PUT /steop/state/:id`                    | `steop.state.put`      |
| `DELETE /steop/state/:id`                 | `steop.state.delete`   |
| `POST /steop/state/:id/incr`              | `steop.state.incr`     |
| `POST /steop/state/:id/reset`             | `steop.state.reset`    |
| `GET /steop/status/:id`                   | `steop.status.get`     |
| `GET /steop/sessions`                     | `steop.session.list`   |
| `GET /steop/sessions/:id`                 | `steop.session.get`    |
| `POST /steop/notify`                      | `steop.notify`         |
| `POST /steop/log`                         | `steop.log.append`     |
| `GET /steop/log`                          | `steop.log.query`      |
| `POST /steop/inbox`                       | `steop.mailbox.send`   |
| `GET /steop/inbox`                        | `steop.mailbox.list`   |

The `X-Steop-Host` / `X-Steop-Project-Dir` headers are no longer read.

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
