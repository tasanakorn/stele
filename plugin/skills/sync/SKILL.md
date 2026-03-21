---
name: sync
description: Pull latest shared team context from Stele into the current session — flat memories and knowledge graph
user-invocable: true
---

# Sync Team Context from Stele

Pull the latest shared knowledge from Stele so you're working with up-to-date team context.

## Procedure

### Step 1: Determine Project Scope

Look for the project's Stele scope in one of these locations (in order):

1. The current project's `CLAUDE.md` — find the "Stele Shared Memory Protocol" section and extract the scope from the `**Scope:**` line
2. If not found, ask the user for the scope to query

### Step 2: Pull Flat Memories

```
recall_memories(scope: ["{scope}", "global"])
```

This retrieves all flat memories for the project scope (including sub-scopes via prefix matching) plus any global shared knowledge.

### Step 3: Pull Knowledge Graph

```
search_nodes(query: "*", scope: ["{scope}", "global"])
```

This retrieves all entities and their observations across the project and global scopes.

### Step 4: Summarize

Present the results to the user in a structured summary:

1. **Active Conventions** — memories tagged `#active` or `#contract`
2. **Recent Decisions** — memories of type `decision`, sorted by most recent
3. **Known Issues / Wisdom** — memories tagged `#wisdom` or `#todo`
4. **Architecture Overview** — entities and their relationships from the knowledge graph
5. **Breaking Changes** — any memories tagged `#breaking` that may need attention

Keep the summary concise. Group related items together. Highlight anything that seems immediately relevant to the user's current task if you can infer it from context.

### Step 5: Offer Follow-up

Ask the user if they want to:
- Drill deeper into any specific area
- Open specific nodes to see their relationships: `open_nodes(names: [...], scope: "{scope}")`
- See the full graph: `read_graph(scope: "{scope}")`
