# MCP Tool Reference

## Overview

Stele exposes 17 MCP tools over Streamable HTTP:

- 7 flat memory tools
- 9 knowledge graph tools
- 1 deprecated bootstrap tool

The MCP endpoint is served at a configurable path (default `/mcp`). Configure Claude Code to connect:

```json
{
  "mcpServers": {
    "stele": {
      "type": "streamableHttp",
      "url": "http://localhost:3100/mcp"
    }
  }
}
```

## Parameter Notes

Parameters that accept arrays use lenient deserialization. Both standard JSON arrays and JSON-encoded strings are accepted:

- JSON array: `["a", "b"]`
- JSON-encoded string: `"[\"a\",\"b\"]"`

Scope parameters additionally accept a bare string for single-scope use:

- Bare string: `"team-a"`
- Array: `["team-a", "global"]`

---

## Flat Memory Tools

### store_memory

Store a new shared memory with scope and tags.

| Parameter   | Type     | Required | Description                                                                               |
| ----------- | -------- | -------- | ----------------------------------------------------------------------------------------- |
| title       | string   | yes      | Title of the memory                                                                       |
| content     | string   | yes      | Content of the memory                                                                     |
| scope       | string   | yes      | Hierarchical scope (e.g. `team-a/frontend`)                                               |
| tags        | string[] | no       | Flat labels for categorization                                                            |
| memory_type | string   | no       | `knowledge`, `decision`, `convention`, `troubleshooting`, `reference`, `other` (default: `knowledge`) |
| author      | string   | no       | Who created this memory                                                                   |

**Returns:** Memory object as JSON.

---

### recall_memories

Search memories by keywords, scope(s), and/or tags.

| Parameter      | Type               | Required | Description                                       |
| -------------- | ------------------ | -------- | ------------------------------------------------- |
| query          | string             | no       | Full-text search query                            |
| scope          | string or string[] | no       | Scope prefix(es) to filter by                     |
| tags           | string[]           | no       | Tags to filter by                                 |
| match_all_tags | boolean            | no       | Require ALL tags to match (default: `false`)      |
| limit          | number             | no       | Max results (default: 20, max: 100)               |

**Returns:** Array of SearchResult objects (Memory fields + `rank`).

---

### get_memory

Retrieve a specific memory by its ID.

| Parameter | Type   | Required | Description          |
| --------- | ------ | -------- | -------------------- |
| id        | string | yes      | ULID of the memory   |

**Returns:** Memory object.

---

### update_memory

Update an existing memory's fields. Only provided fields are updated.

| Parameter   | Type     | Required | Description                              |
| ----------- | -------- | -------- | ---------------------------------------- |
| id          | string   | yes      | ULID of the memory                       |
| title       | string   | no       | New title                                |
| content     | string   | no       | New content                              |
| scope       | string   | no       | New scope                                |
| tags        | string[] | no       | New tags — replaces all existing tags    |
| memory_type | string   | no       | New memory type                          |

**Returns:** Updated Memory object.

---

### forget_memory

Delete a memory by its ID.

| Parameter | Type   | Required | Description        |
| --------- | ------ | -------- | ------------------ |
| id        | string | yes      | ULID of the memory |

**Returns:** Confirmation message string.

---

### list_scopes

List all scopes with memory counts.

| Parameter | Type   | Required | Description              |
| --------- | ------ | -------- | ------------------------ |
| prefix    | string | no       | Filter scopes by prefix  |

**Returns:** Array of `{scope, count}`.

---

### list_tags

List all tags with memory counts.

| Parameter | Type               | Required | Description                              |
| --------- | ------------------ | -------- | ---------------------------------------- |
| scope     | string or string[] | no       | Filter tags within the given scope(s)    |

**Returns:** Array of `{tag, count}`.

---

## Knowledge Graph Tools

### create_entities

Create nodes in the knowledge graph. Idempotent — if an entity with the same name already exists in the scope, its observations are appended rather than creating a duplicate.

| Parameter | Type          | Required | Description                                                              |
| --------- | ------------- | -------- | ------------------------------------------------------------------------ |
| entities  | EntityInput[] | yes      | Array of `{name, entity_type, observations?}` objects                   |
| scope     | string        | yes      | Scope for all entities                                                   |

**EntityInput fields:**

| Field        | Type     | Required | Description                                     |
| ------------ | -------- | -------- | ----------------------------------------------- |
| name         | string   | yes      | Entity name (unique within scope)               |
| entity_type  | string   | yes      | Type (e.g. `person`, `component`, `service`)    |
| observations | string[] | no       | Initial atomic facts about this entity          |

**Returns:** Array of `{name, id, created}`.

---

### create_relations

Create directed edges between entities. Both entities must already exist in the given scope. Idempotent.

| Parameter | Type           | Required | Description                                                    |
| --------- | -------------- | -------- | -------------------------------------------------------------- |
| relations | RelationInput[] | yes     | Array of `{from, to, relation_type}` objects                  |
| scope     | string         | yes      | Scope where the entities exist                                 |

**RelationInput fields:**

| Field         | Type   | Required | Description                                          |
| ------------- | ------ | -------- | ---------------------------------------------------- |
| from          | string | yes      | Name of the source entity                            |
| to            | string | yes      | Name of the target entity                            |
| relation_type | string | yes      | Edge label (e.g. `depends_on`, `owns`, `calls`)      |

**Returns:** Array of `{from, to, relation_type, id, created}`.

---

### add_observations

Add atomic facts to an existing entity.

| Parameter    | Type     | Required | Description                              |
| ------------ | -------- | -------- | ---------------------------------------- |
| entity_name  | string   | yes      | Name of the entity                       |
| scope        | string   | yes      | Scope where the entity exists            |
| observations | string[] | yes      | Facts to add                             |

**Returns:** Updated Entity object.

---

### delete_entities

Delete nodes from the knowledge graph. Cascades to observations and relations.

| Parameter    | Type     | Required | Description                          |
| ------------ | -------- | -------- | ------------------------------------ |
| entity_names | string[] | yes      | Names of entities to delete          |
| scope        | string   | yes      | Scope where the entities exist       |

**Returns:** `{deleted: string[], not_found: string[]}`.

---

### delete_observations

Remove specific observations from an entity by exact content match.

| Parameter    | Type     | Required | Description                                  |
| ------------ | -------- | -------- | -------------------------------------------- |
| entity_name  | string   | yes      | Name of the entity                           |
| scope        | string   | yes      | Scope where the entity exists                |
| observations | string[] | yes      | Exact observation content strings to remove  |

**Returns:** Updated Entity object.

---

### delete_relations

Delete specific edges from the knowledge graph.

| Parameter | Type            | Required | Description                                      |
| --------- | --------------- | -------- | ------------------------------------------------ |
| relations | RelationInput[] | yes      | Array of `{from, to, relation_type}` to delete   |
| scope     | string          | yes      | Scope where the entities exist                   |

**Returns:** `{deleted: string[], not_found: string[]}`.

---

### read_graph

Read the full knowledge graph (all entities, observations, and relations) for one or more scopes.

| Parameter | Type               | Required | Description                               |
| --------- | ------------------ | -------- | ----------------------------------------- |
| scope     | string or string[] | yes      | Scope(s) to read (prefix-matched)         |

**Returns:** Graph object `{entities: Entity[], relations: Relation[]}`.

---

### search_nodes

Full-text search across entity names and observation content.

| Parameter | Type               | Required | Description                               |
| --------- | ------------------ | -------- | ----------------------------------------- |
| query     | string             | yes      | Search query                              |
| scope     | string or string[] | no       | Scope filter (prefix-matched)             |
| limit     | number             | no       | Max results (default: 20, max: 100)       |

**Returns:** Array of EntitySearchResult `{entity, rank}`.

---

### open_nodes

Fetch specific entities by name along with their direct neighbor relations.

| Parameter | Type               | Required | Description                                     |
| --------- | ------------------ | -------- | ----------------------------------------------- |
| names     | string[]           | yes      | Entity names to open                            |
| scope     | string or string[] | yes      | Scope(s) where the entities exist               |

**Returns:** Graph containing the named entities and any directly connected neighbor entities and relations.

---

## Deprecated Tools

### bootstrap_project

Generates a CLAUDE.md snippet for a project that describes how to use Stele's flat memory and knowledge graph. Still functional but not recommended for new projects.

**Replaced by:** the Stele plugin's `/stele:bootstrap` skill.

| Parameter    | Type   | Required | Description                                                                      |
| ------------ | ------ | -------- | -------------------------------------------------------------------------------- |
| project_name | string | yes      | Name of the project                                                              |
| parent_scope | string | no       | Parent scope (e.g. `team-a`). Project scope becomes `parent_scope/project_name` |
| project_type | string | no       | `web-app`, `library`, `api`, `monorepo`, `data-pipeline` or omit for generic    |

**Returns:** Formatted CLAUDE.md protocol section as a string.
