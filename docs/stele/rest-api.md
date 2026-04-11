# REST API Reference

## Overview

Stele exposes a JSON REST API at `/api/v1`. CORS is fully permissive (all origins allowed). All error responses use the format:

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
