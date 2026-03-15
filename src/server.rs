use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::db::{self, DbPool};
use crate::models::{Memory, MemoryType};
use crate::query::SearchParams;

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
    /// Flat labels for multi-perspective categorization (e.g. ["vue", "auth"])
    #[serde(default)]
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
    /// Tags to filter by
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
    /// New tags - replaces all existing tags (optional)
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
             Tags are flat labels for multi-perspective categorization."
                .to_string(),
        );
        info
    }
}
