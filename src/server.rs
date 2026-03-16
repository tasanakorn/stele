use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::models::{Memory, MemoryType};
use crate::query::SearchParams;
use crate::serde_helpers::{string_or_vec, string_or_vec_opt};
use serde::Serialize;

#[derive(Clone)]
pub struct SteleServer {
    pub db: DbPool,
    tool_router: ToolRouter<Self>,
}

impl SteleServer {
    pub fn new(db: DbPool) -> Self {
        Self {
            db,
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StoreMemoryParams {
    /// Title of the memory
    pub title: String,
    /// Content of the memory
    pub content: String,
    /// Hierarchical scope (e.g. "team-a/frontend"). Used for prefix-matched filtering.
    pub scope: String,
    /// JSON array of flat labels for multi-perspective categorization, e.g. ["vue", "auth"]
    #[serde(default, deserialize_with = "string_or_vec")]
    pub tags: Vec<String>,
    /// Type of memory: knowledge, decision, convention, troubleshooting, reference, other
    pub memory_type: Option<String>,
    /// Who created this memory
    pub author: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecallMemoriesParams {
    /// Full-text search query
    pub query: Option<String>,
    /// Scope prefix to filter by (e.g. "team-a" matches "team-a/frontend" too)
    pub scope: Option<String>,
    /// JSON array of tags to filter by, e.g. ["vue", "auth"]
    #[serde(default, deserialize_with = "string_or_vec_opt")]
    pub tags: Option<Vec<String>>,
    /// If true, memory must have ALL specified tags. Default: false (any match).
    #[serde(default)]
    pub match_all_tags: bool,
    /// Maximum number of results (default: 20, max: 100)
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMemoryParams {
    /// The ULID of the memory to retrieve
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMemoryParams {
    /// The ULID of the memory to update
    pub id: String,
    /// New title (optional)
    pub title: Option<String>,
    /// New content (optional)
    pub content: Option<String>,
    /// New scope (optional)
    pub scope: Option<String>,
    /// JSON array of new tags — replaces all existing tags, e.g. ["vue", "auth"]
    #[serde(default, deserialize_with = "string_or_vec_opt")]
    pub tags: Option<Vec<String>>,
    /// New memory type (optional)
    pub memory_type: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ForgetMemoryParams {
    /// The ULID of the memory to delete
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListScopesParams {
    /// Optional prefix to filter scopes
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTagsParams {
    /// Optional scope to filter tags within
    pub scope: Option<String>,
}

// ── Knowledge Graph result structs ──

#[derive(Debug, Serialize)]
struct EntityCreatedResult {
    name: String,
    id: String,
    created: bool,
}

#[derive(Debug, Serialize)]
struct RelationCreatedResult {
    from: String,
    to: String,
    relation_type: String,
    id: String,
    created: bool,
}

// ── Knowledge Graph param structs ──

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EntityInput {
    /// Name of the entity (unique within scope)
    pub name: String,
    /// Type of entity (e.g. "person", "component", "service", "concept")
    pub entity_type: String,
    /// JSON array of initial observations (atomic facts) about this entity, e.g. ["fact one", "fact two"]
    #[serde(default, deserialize_with = "string_or_vec")]
    pub observations: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntitiesParams {
    /// JSON array of entities to create. Each element: {name, entity_type, observations?}
    #[serde(deserialize_with = "string_or_vec")]
    pub entities: Vec<EntityInput>,
    /// Hierarchical scope for these entities
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RelationInput {
    /// Name of the source entity
    pub from: String,
    /// Name of the target entity
    pub to: String,
    /// Type of relation (e.g. "depends_on", "owns", "uses", "calls")
    pub relation_type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateRelationsParams {
    /// JSON array of relations to create. Each element: {from, to, relation_type}
    #[serde(deserialize_with = "string_or_vec")]
    pub relations: Vec<RelationInput>,
    /// Scope where the entities exist
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddObservationsParams {
    /// Name of the entity to add observations to
    pub entity_name: String,
    /// Scope where the entity exists
    pub scope: String,
    /// JSON array of observations (atomic facts) to add, e.g. ["fact one", "fact two"]
    #[serde(deserialize_with = "string_or_vec")]
    pub observations: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteEntitiesParams {
    /// JSON array of entity names to delete (cascades observations and relations), e.g. ["EntityA", "EntityB"]
    #[serde(deserialize_with = "string_or_vec")]
    pub entity_names: Vec<String>,
    /// Scope where the entities exist
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteObservationsParams {
    /// Name of the entity to remove observations from
    pub entity_name: String,
    /// Scope where the entity exists
    pub scope: String,
    /// JSON array of exact observation content strings to remove, e.g. ["fact one", "fact two"]
    #[serde(deserialize_with = "string_or_vec")]
    pub observations: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteRelationsParams {
    /// JSON array of relations to delete. Each element: {from, to, relation_type}
    #[serde(deserialize_with = "string_or_vec")]
    pub relations: Vec<RelationInput>,
    /// Scope where the entities exist
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadGraphParams {
    /// Scope to read the graph from (prefix-matched)
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchNodesParams {
    /// Full-text search query
    pub query: String,
    /// Optional scope prefix to filter by
    pub scope: Option<String>,
    /// Maximum number of results (default: 20, max: 100)
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenNodesParams {
    /// JSON array of entity names to open, e.g. ["EntityA", "EntityB"]
    #[serde(deserialize_with = "string_or_vec")]
    pub names: Vec<String>,
    /// Scope where the entities exist
    pub scope: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BootstrapProjectParams {
    /// Name of the project
    pub project_name: String,
    /// Parent scope (e.g. "team-a"). If provided, project scope becomes parent_scope/project_name
    pub parent_scope: Option<String>,
    /// Type of project (e.g. "web-app", "library", "api", "monorepo", "data-pipeline")
    pub project_type: Option<String>,
}

#[tool_router]
impl SteleServer {
    #[tool(description = "Store a new shared memory with scope and tags")]
    async fn store_memory(&self, Parameters(params): Parameters<StoreMemoryParams>) -> String {
        let now = chrono::Utc::now().to_rfc3339();
        let id = ulid::Ulid::new().to_string();

        let memory = Memory {
            id: id.clone(),
            title: params.title,
            content: params.content,
            memory_type: MemoryType::from_str(
                params.memory_type.as_deref().unwrap_or("knowledge"),
            ),
            scope: params.scope,
            author: params.author,
            tags: params.tags,
            created_at: now.clone(),
            updated_at: now,
        };

        let conn = self.db.lock().await;
        match db::insert_memory(&conn, &memory) {
            Ok(()) => serde_json::to_string_pretty(&memory).unwrap_or_default(),
            Err(e) => format!("Error storing memory: {e}"),
        }
    }

    #[tool(description = "Search memories by keywords, scope, and/or tags")]
    async fn recall_memories(
        &self,
        Parameters(params): Parameters<RecallMemoriesParams>,
    ) -> String {
        let search = SearchParams {
            query: params.query,
            scope: params.scope,
            tags: params.tags,
            match_all_tags: params.match_all_tags,
            limit: params.limit,
        };

        let conn = self.db.lock().await;
        match db::search_memories(&conn, &search) {
            Ok(results) => serde_json::to_string_pretty(&results).unwrap_or_default(),
            Err(e) => format!("Error searching memories: {e}"),
        }
    }

    #[tool(description = "Retrieve a specific memory by its ID")]
    async fn get_memory(&self, Parameters(params): Parameters<GetMemoryParams>) -> String {
        let conn = self.db.lock().await;
        match db::get_memory(&conn, &params.id) {
            Ok(Some(m)) => serde_json::to_string_pretty(&m).unwrap_or_default(),
            Ok(None) => format!("Memory not found: {}", params.id),
            Err(e) => format!("Error getting memory: {e}"),
        }
    }

    #[tool(description = "Update an existing memory's title, content, scope, tags, or type")]
    async fn update_memory(&self, Parameters(params): Parameters<UpdateMemoryParams>) -> String {
        let conn = self.db.lock().await;
        match db::update_memory(
            &conn,
            &params.id,
            params.title.as_deref(),
            params.content.as_deref(),
            params.scope.as_deref(),
            params.tags.as_deref(),
            params.memory_type.as_deref(),
        ) {
            Ok(true) => match db::get_memory(&conn, &params.id) {
                Ok(Some(m)) => serde_json::to_string_pretty(&m).unwrap_or_default(),
                Ok(None) => format!("Memory not found after update: {}", params.id),
                Err(e) => format!("Error fetching updated memory: {e}"),
            },
            Ok(false) => format!("Memory not found: {}", params.id),
            Err(e) => format!("Error updating memory: {e}"),
        }
    }

    #[tool(description = "Delete a memory by its ID")]
    async fn forget_memory(&self, Parameters(params): Parameters<ForgetMemoryParams>) -> String {
        let conn = self.db.lock().await;
        match db::delete_memory(&conn, &params.id) {
            Ok(true) => format!("Memory deleted: {}", params.id),
            Ok(false) => format!("Memory not found: {}", params.id),
            Err(e) => format!("Error deleting memory: {e}"),
        }
    }

    #[tool(description = "List all scopes with memory counts, optionally filtered by prefix")]
    async fn list_scopes(&self, Parameters(params): Parameters<ListScopesParams>) -> String {
        let conn = self.db.lock().await;
        match db::list_scopes(&conn, params.prefix.as_deref()) {
            Ok(scopes) => serde_json::to_string_pretty(&scopes).unwrap_or_default(),
            Err(e) => format!("Error listing scopes: {e}"),
        }
    }

    #[tool(description = "List all tags with memory counts, optionally filtered by scope")]
    async fn list_tags(&self, Parameters(params): Parameters<ListTagsParams>) -> String {
        let conn = self.db.lock().await;
        match db::list_tags(&conn, params.scope.as_deref()) {
            Ok(tags) => serde_json::to_string_pretty(&tags).unwrap_or_default(),
            Err(e) => format!("Error listing tags: {e}"),
        }
    }

    // ── Knowledge Graph tools ──

    #[tool(description = "Create entities (nodes) in the knowledge graph. Idempotent — existing entities get observations appended.")]
    async fn create_entities(&self, Parameters(params): Parameters<CreateEntitiesParams>) -> String {
        let conn = self.db.lock().await;
        let mut results: Vec<EntityCreatedResult> = Vec::new();

        for input in &params.entities {
            match db::insert_entity(&conn, &input.name, &input.entity_type, &params.scope, &input.observations) {
                Ok((id, created)) => results.push(EntityCreatedResult {
                    name: input.name.clone(),
                    id,
                    created,
                }),
                Err(e) => return format!("Error creating entity '{}': {e}", input.name),
            }
        }

        serde_json::to_string_pretty(&results).unwrap_or_default()
    }

    #[tool(description = "Create directed relations (edges) between entities in the knowledge graph. Both entities must exist in the given scope.")]
    async fn create_relations(&self, Parameters(params): Parameters<CreateRelationsParams>) -> String {
        let conn = self.db.lock().await;
        let mut results: Vec<RelationCreatedResult> = Vec::new();

        for input in &params.relations {
            match db::insert_relation(&conn, &input.from, &input.to, &input.relation_type, &params.scope) {
                Ok(Some((id, created))) => results.push(RelationCreatedResult {
                    from: input.from.clone(),
                    to: input.to.clone(),
                    relation_type: input.relation_type.clone(),
                    id,
                    created,
                }),
                Ok(None) => return format!(
                    "Error: entity '{}' or '{}' not found in scope '{}'",
                    input.from, input.to, params.scope
                ),
                Err(e) => return format!("Error creating relation: {e}"),
            }
        }

        serde_json::to_string_pretty(&results).unwrap_or_default()
    }

    #[tool(description = "Add observations (atomic facts) to an existing entity")]
    async fn add_observations(&self, Parameters(params): Parameters<AddObservationsParams>) -> String {
        let conn = self.db.lock().await;
        match db::insert_observations(&conn, &params.entity_name, &params.scope, &params.observations) {
            Ok(Some(entity)) => serde_json::to_string_pretty(&entity).unwrap_or_default(),
            Ok(None) => format!("Entity '{}' not found in scope '{}'", params.entity_name, params.scope),
            Err(e) => format!("Error adding observations: {e}"),
        }
    }

    #[tool(description = "Delete entities from the knowledge graph. Cascades to observations and relations.")]
    async fn delete_entities(&self, Parameters(params): Parameters<DeleteEntitiesParams>) -> String {
        let conn = self.db.lock().await;
        let mut deleted = Vec::new();
        let mut not_found = Vec::new();

        for name in &params.entity_names {
            match db::delete_entity(&conn, name, &params.scope) {
                Ok(true) => deleted.push(name.clone()),
                Ok(false) => not_found.push(name.clone()),
                Err(e) => return format!("Error deleting entity '{name}': {e}"),
            }
        }

        serde_json::to_string_pretty(&serde_json::json!({
            "deleted": deleted,
            "not_found": not_found,
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Remove specific observations from an entity by exact content match")]
    async fn delete_observations(&self, Parameters(params): Parameters<DeleteObservationsParams>) -> String {
        let conn = self.db.lock().await;
        match db::delete_observations(&conn, &params.entity_name, &params.scope, &params.observations) {
            Ok(Some(entity)) => serde_json::to_string_pretty(&entity).unwrap_or_default(),
            Ok(None) => format!("Entity '{}' not found in scope '{}'", params.entity_name, params.scope),
            Err(e) => format!("Error deleting observations: {e}"),
        }
    }

    #[tool(description = "Delete specific relations from the knowledge graph")]
    async fn delete_relations(&self, Parameters(params): Parameters<DeleteRelationsParams>) -> String {
        let conn = self.db.lock().await;
        let mut deleted = Vec::new();
        let mut not_found = Vec::new();

        for input in &params.relations {
            match db::delete_relation(&conn, &input.from, &input.to, &input.relation_type, &params.scope) {
                Ok(true) => deleted.push(format!("{} --[{}]--> {}", input.from, input.relation_type, input.to)),
                Ok(false) => not_found.push(format!("{} --[{}]--> {}", input.from, input.relation_type, input.to)),
                Err(e) => return format!("Error deleting relation: {e}"),
            }
        }

        serde_json::to_string_pretty(&serde_json::json!({
            "deleted": deleted,
            "not_found": not_found,
        }))
        .unwrap_or_default()
    }

    #[tool(description = "Read the full knowledge graph for a scope (all entities, observations, and relations)")]
    async fn read_graph(&self, Parameters(params): Parameters<ReadGraphParams>) -> String {
        let conn = self.db.lock().await;
        match db::read_graph(&conn, &params.scope) {
            Ok(graph) => serde_json::to_string_pretty(&graph).unwrap_or_default(),
            Err(e) => format!("Error reading graph: {e}"),
        }
    }

    #[tool(description = "Search the knowledge graph by entity name or observation content using full-text search")]
    async fn search_nodes(&self, Parameters(params): Parameters<SearchNodesParams>) -> String {
        let limit = params.limit.unwrap_or(20).min(100);
        let conn = self.db.lock().await;
        match db::search_entities(&conn, &params.query, params.scope.as_deref(), limit) {
            Ok(results) => serde_json::to_string_pretty(&results).unwrap_or_default(),
            Err(e) => format!("Error searching nodes: {e}"),
        }
    }

    #[tool(description = "Open specific entities by name, returning them with their observations and direct neighbor relations")]
    async fn open_nodes(&self, Parameters(params): Parameters<OpenNodesParams>) -> String {
        let conn = self.db.lock().await;
        match db::open_entities(&conn, &params.names, &params.scope) {
            Ok(graph) => serde_json::to_string_pretty(&graph).unwrap_or_default(),
            Err(e) => format!("Error opening nodes: {e}"),
        }
    }

    #[tool(description = "Generate a CLAUDE.md snippet for a project that teaches Claude Code how to use Stele's flat memory and knowledge graph together")]
    async fn bootstrap_project(&self, Parameters(params): Parameters<BootstrapProjectParams>) -> String {
        let scope = match &params.parent_scope {
            Some(parent) => format!("{parent}/{}", params.project_name),
            None => params.project_name.clone(),
        };

        let project_type = params.project_type.as_deref().unwrap_or("general");

        let entity_types = match project_type {
            "web-app" | "frontend" => "component, page, service, store, api-endpoint, library, person",
            "api" | "backend" => "service, endpoint, middleware, model, database, queue, person",
            "library" | "sdk" => "module, function, type, trait, interface, example, person",
            "monorepo" => "package, service, library, shared-module, pipeline, person",
            "data-pipeline" | "ml" => "pipeline, model, dataset, feature, transform, scheduler, person",
            _ => "component, service, module, concept, person, tool, dependency",
        };

        let relation_types = match project_type {
            "web-app" | "frontend" => "imports, renders, calls, depends_on, owned_by, routes_to",
            "api" | "backend" => "calls, depends_on, reads_from, writes_to, owned_by, authenticates",
            "library" | "sdk" => "exports, depends_on, implements, extends, documented_in, owned_by",
            "monorepo" => "depends_on, imports, publishes, consumes, owned_by, deployed_with",
            "data-pipeline" | "ml" => "feeds_into, transforms, reads_from, writes_to, trains, owned_by",
            _ => "depends_on, calls, owns, uses, implements, extends",
        };

        let tag_suggestions = match project_type {
            "web-app" | "frontend" => "- `#ui` — UI/UX decisions\n- `#state` — state management patterns\n- `#routing` — navigation and routing",
            "api" | "backend" => "- `#endpoint` — API endpoint documentation\n- `#schema` — data model and schema changes\n- `#auth` — authentication and authorization",
            "library" | "sdk" => "- `#public-api` — public interface decisions\n- `#semver` — version compatibility notes\n- `#docs` — documentation requirements",
            "monorepo" => "- `#cross-pkg` — cross-package concerns\n- `#build` — build system and CI changes\n- `#shared` — shared module updates",
            "data-pipeline" | "ml" => "- `#data-quality` — data validation rules\n- `#model` — model architecture decisions\n- `#pipeline` — pipeline configuration",
            _ => "- `#architecture` — architectural decisions\n- `#integration` — integration points\n- `#infra` — infrastructure concerns",
        };

        format!(
            r##"## Stele Shared Memory Protocol — {project_name}

**Scope:** `{scope}` | **Type:** {project_type}
**Server:** [Stele](https://github.com/tasanakorn/stele) — shared memory for multi-agent Claude Code

---

### 1. Hybrid Storage Strategy

Stele provides two complementary memory systems. Use both.

#### Flat Memory (Scoped)
Operational facts, decisions, conventions, troubleshooting notes. Key-value-like prose entries with scope + tags.

```
store_memory(
  scope: "{scope}",
  title: "DB Port",
  content: "PostgreSQL on port 5432, connection pool max 20",
  memory_type: "convention",
  tags: ["#active", "#contract"]
)

recall_memories(query: "database port", scope: "{scope}")
```

#### Knowledge Graph (KG)
Structural relationships between entities — services, components, people, dependencies.

```
create_entities(entities: [
  {{name: "OrderService", entity_type: "service", observations: ["gRPC service, handles order lifecycle"]}},
  {{name: "PaymentService", entity_type: "service", observations: ["Stripe integration, PCI-compliant"]}}
], scope: "{scope}")

create_relations(relations: [
  {{from: "OrderService", to: "PaymentService", relation_type: "depends_on"}}
], scope: "{scope}")
```

**Rule of thumb:** If it's a **fact or note** → flat memory. If it's a **thing with relationships** → knowledge graph.

#### When to use which

| Use flat memories for...            | Use knowledge graph for...          |
| ----------------------------------- | ----------------------------------- |
| Decisions and their rationale       | Architecture and component maps     |
| Coding conventions and style rules  | People and ownership                |
| Troubleshooting steps               | Dependencies between services       |
| External references and links       | Data flow and call chains           |
| Onboarding notes                    | Entity facts (observations)         |

---

### 2. Knowledge Synchronization & Consistency

#### On Boot (every task start)
At the beginning of every task, pull the latest shared state. **Do not assume you know the current state.**

```
recall_memories(scope: "{scope}")
search_nodes(query: "*", scope: "{scope}")
```

#### Dependency Awareness
Before proposing architectural changes, check what depends on the module you're changing:

```
open_nodes(names: ["ModuleName"], scope: "{scope}")
read_graph(scope: "{scope}")
```

#### Consistency Rules
- Use the top-level scope `{top_scope}` for workspace-wide standards shared across all sub-projects.
- If a local rule in `{scope}` conflicts with a rule in `{top_scope}`, flag it with a `#conflict` tag.
- When creating a new service or module, call `bootstrap_project` with `parent_scope: "{scope}"` — use the returned content for the local CLAUDE.md. Do not hallucinate setup instructions.

---

### 3. Update-on-Change Protocol (Autonomous — no permission needed)

You MUST update remote memory immediately when any of the following change. Do not defer or ask for permission.

#### Contract Changes
If an API signature, env var, or shared interface changes:
```
store_memory(scope: "{scope}", title: "API change: ...", content: "...", tags: ["#contract", "#breaking"])
add_observations(entity_name: "ServiceName", scope: "{scope}", observations: ["New endpoint POST /v2/orders added"])
```

#### Lessons Learned
If a non-obvious bug is fixed, record it so other agents don't repeat the mistake:
```
add_observations(entity_name: "ServiceName", scope: "{scope}", observations: ["Gotcha: must set Content-Type header explicitly for multipart"])
store_memory(scope: "{scope}", title: "Fix: multipart upload", content: "...", tags: ["#wisdom"])
```

#### Relationship Discovery
If you discover Service A calls Service B, record it immediately:
```
create_relations(relations: [{{from: "A", to: "B", relation_type: "calls"}}], scope: "{scope}")
```

---

### 4. Tagging Convention

Always tag facts to enable cross-agent/cross-machine search:

| Tag          | Meaning                                                |
| ------------ | ------------------------------------------------------ |
| `#active`    | Currently implemented and enforced rules               |
| `#todo`      | Technical debt or pending migrations                   |
| `#contract`  | Inter-service API definitions and shared interfaces    |
| `#breaking`  | Changes that require other agents/services to update   |
| `#wisdom`    | Non-obvious technical discoveries and gotchas          |
| `#conflict`  | Local rule that conflicts with a workspace-level rule  |
| `#v[N]`      | Version-specific notes (e.g. `#v2-migration`)          |

Project-type-specific tags:
{tag_suggestions}

---

### 5. Scope Guide

Scopes are hierarchical, like message queue topics. Queries use **prefix matching**.

| Scope                | What it covers                       | Query `{top_scope}` matches? |
| -------------------- | ------------------------------------ | ---------------------------- |
| `{top_scope}`        | Workspace-wide standards             | Yes                          |
| `{scope}`            | This project                         | {scope_match}                |
| `{scope}/backend`    | Backend-specific knowledge           | {scope_match}                |
| `{scope}/frontend`   | Frontend-specific knowledge          | {scope_match}                |

Examples:
- `recall_memories(scope: "{scope}")` — matches `{scope}`, `{scope}/backend`, `{scope}/frontend`, etc.
- `recall_memories(scope: "{top_scope}")` — matches everything in the workspace.

---

### 6. Suggested Entity & Relation Types

**Entity types:** `{entity_types}`

**Relation types:** `{relation_types}`

---

### 7. First-Time Setup

If this is a new project with no existing memories:
1. Ask the user for the top-level workspace scope (if not already known).
2. Ask if there are sub-scopes to create (e.g. `{scope}/backend`, `{scope}/frontend`).
3. Create initial entities for the major components you can identify from the codebase.
4. Store any conventions or decisions the user mentions during onboarding.
"##,
            project_name = params.project_name,
            project_type = project_type,
            scope = scope,
            top_scope = params.parent_scope.as_deref().unwrap_or(&scope),
            scope_match = if params.parent_scope.is_some() { "Yes" } else { "N/A (top-level)" },
            entity_types = entity_types,
            relation_types = relation_types,
            tag_suggestions = tag_suggestions,
        )
    }
}

#[tool_handler]
impl ServerHandler for SteleServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.instructions = Some(
            "Stele is a shared memory layer for teams using Claude Code. \
             Use store_memory to save knowledge, decisions, conventions, and troubleshooting notes. \
             Use recall_memories to search by keywords, scope, and tags. \
             Scopes are hierarchical (e.g. 'team-a/frontend') and prefix-matched. \
             Tags are flat labels for multi-perspective categorization. \
             The knowledge graph (create_entities, create_relations, search_nodes, open_nodes, read_graph) \
             stores structural relationships between components, services, people, and dependencies. \
             Use bootstrap_project to generate a full operational protocol for a new project — \
             it produces a comprehensive CLAUDE.md section covering hybrid storage strategy, \
             knowledge synchronization, update-on-change rules, tagging conventions, and scope guidance."
                .to_string(),
        );
        info
    }
}
