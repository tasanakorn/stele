---
name: checkpoint
description: Save session findings, decisions, and discoveries back to Stele shared memory
user-invocable: true
---

# Checkpoint Session to Stele

Review the current conversation and persist important findings to Stele so other team members and future sessions benefit.

## Procedure

### Step 1: Determine Project Scope

Look for the project's Stele scope in the current project's `CLAUDE.md` — find the "Stele Shared Memory Protocol" section and extract the scope from the `**Scope:**` line. If not found, ask the user.

### Step 2: Review Conversation

Scan the current conversation for:

1. **Decisions made** — architectural choices, technology selections, convention agreements
2. **Bugs fixed** — especially non-obvious root causes or gotchas
3. **Conventions established** — coding standards, naming patterns, process rules
4. **Architecture changes** — new components, modified dependencies, removed services
5. **Contract changes** — API modifications, env var changes, shared interface updates

### Step 3: Classify and Store

For each finding, determine the storage type and apply appropriate tags:

#### Flat Memory (facts, notes, decisions)

```
store_memory(
  scope: "{scope}",
  title: "{concise title}",
  content: "{detailed description with context}",
  memory_type: "{knowledge|decision|convention|troubleshooting|reference}",
  tags: ["{appropriate tags}"]
)
```

**Tag selection guide:**
- `#active` — rule or convention currently in effect
- `#contract` — API or interface definition
- `#breaking` — change that affects other services/agents
- `#wisdom` — non-obvious discovery or gotcha
- `#todo` — identified technical debt

#### Knowledge Graph (things with relationships)

For new components or services discovered:
```
create_entities(
  entities: [{name: "...", entity_type: "...", observations: ["..."]}],
  scope: "{scope}"
)
```

For new observations about existing entities:
```
add_observations(
  entity_name: "...",
  scope: "{scope}",
  observations: ["..."]
)
```

For discovered relationships:
```
create_relations(
  relations: [{from: "...", to: "...", relation_type: "..."}],
  scope: "{scope}"
)
```

### Step 4: Confirm

Present a summary of what was saved:
- Number of flat memories stored
- Number of entities created or updated
- Number of relations created
- List each item briefly (title + tags for memories, name + type for entities)

Ask the user if anything was missed or if they want to add additional context to any item.
