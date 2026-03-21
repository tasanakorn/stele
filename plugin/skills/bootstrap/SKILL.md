---
name: bootstrap
description: Initialize a project with Stele shared memory — creates scope, seeds entities, and generates CLAUDE.md protocol section
user-invocable: true
---

# Bootstrap Project with Stele

Set up a project to use Stele shared memory. This skill replaces the deprecated `bootstrap_project` MCP tool with an interactive, side-effect-producing workflow.

## Parameters

Collect the following from the user (or from skill arguments):

1. **project_name** (required) — Name of the project
2. **parent_scope** (optional) — Parent scope (e.g. "acme"). If provided, project scope becomes `parent_scope/project_name`
3. **project_type** (optional) — One of: `web-app`, `frontend`, `api`, `backend`, `library`, `sdk`, `monorepo`, `data-pipeline`, `ml`, or `general` (default)

If any required parameter is missing, ask the user before proceeding.

## Procedure

### Step 1: Compute Scope

If parent_scope is provided:
- scope = `{parent_scope}/{project_name}`
- top_scope = `{parent_scope}`

Otherwise:
- scope = `{project_name}`
- top_scope = `{project_name}`

### Step 2: Look Up Project-Type Mappings

Use the following tables to determine entity_types, relation_types, and tag_suggestions based on project_type:

**Entity Types:**

| Project Type             | Entity Types                                                          |
| ------------------------ | --------------------------------------------------------------------- |
| `web-app`, `frontend`    | component, page, service, store, api-endpoint, library, person        |
| `api`, `backend`         | service, endpoint, middleware, model, database, queue, person         |
| `library`, `sdk`         | module, function, type, trait, interface, example, person             |
| `monorepo`               | package, service, library, shared-module, pipeline, person            |
| `data-pipeline`, `ml`    | pipeline, model, dataset, feature, transform, scheduler, person      |
| `general` (default)      | component, service, module, concept, person, tool, dependency         |

**Relation Types:**

| Project Type             | Relation Types                                                        |
| ------------------------ | --------------------------------------------------------------------- |
| `web-app`, `frontend`    | imports, renders, calls, depends_on, owned_by, routes_to              |
| `api`, `backend`         | calls, depends_on, reads_from, writes_to, owned_by, authenticates     |
| `library`, `sdk`         | exports, depends_on, implements, extends, documented_in, owned_by     |
| `monorepo`               | depends_on, imports, publishes, consumes, owned_by, deployed_with     |
| `data-pipeline`, `ml`    | feeds_into, transforms, reads_from, writes_to, trains, owned_by      |
| `general` (default)      | depends_on, calls, owns, uses, implements, extends                    |

**Tag Suggestions:**

| Project Type             | Tags                                                                                                                  |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| `web-app`, `frontend`    | `#ui` — UI/UX decisions, `#state` — state management patterns, `#routing` — navigation and routing                    |
| `api`, `backend`         | `#endpoint` — API endpoint documentation, `#schema` — data model and schema changes, `#auth` — authentication         |
| `library`, `sdk`         | `#public-api` — public interface decisions, `#semver` — version compatibility notes, `#docs` — documentation           |
| `monorepo`               | `#cross-pkg` — cross-package concerns, `#build` — build system and CI changes, `#shared` — shared module updates      |
| `data-pipeline`, `ml`    | `#data-quality` — data validation rules, `#model` — model architecture decisions, `#pipeline` — pipeline configuration |
| `general` (default)      | `#architecture` — architectural decisions, `#integration` — integration points, `#infra` — infrastructure concerns     |

### Step 3: Seed Stele Database

Create the project entity in the knowledge graph:

```
create_entities(
  entities: [{
    name: "{project_name}",
    entity_type: "project",
    observations: [
      "Project type: {project_type}",
      "Scope: {scope}",
      "Bootstrapped on {today's date}"
    ]
  }],
  scope: "{scope}"
)
```

Store the project conventions as a flat memory:

```
store_memory(
  scope: "{scope}",
  title: "Stele protocol for {project_name}",
  content: "Project bootstrapped with scope '{scope}', type '{project_type}'. Uses hybrid storage: flat memories for facts/notes, knowledge graph for things with relationships. Tagging convention: #active, #todo, #contract, #breaking, #wisdom, #conflict.",
  memory_type: "convention",
  tags: ["#active", "#contract"]
)
```

If parent_scope is provided and a parent project entity exists, create a relation:

```
create_relations(
  relations: [{
    from: "{project_name}",
    to: "{parent project entity name}",
    relation_type: "belongs_to"
  }],
  scope: "{top_scope}"
)
```

### Step 4: Generate CLAUDE.md Section

Generate a markdown section and write it into the project's CLAUDE.md file. If CLAUDE.md already exists, append this section. If it doesn't exist, create it with this section.

The generated section MUST follow this structure exactly, substituting the computed values from steps 1-2. Read the reference template at `references/claude-md-template.md` for the full content.

### Step 5: Confirm

Tell the user:
- The scope that was created
- The entity and memory that were seeded
- Where the CLAUDE.md section was written
- Remind them to commit the CLAUDE.md changes
- If Stele MCP is not yet configured, suggest running `/stele:install` to set it up
