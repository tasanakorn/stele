use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::db::{self, DbPool};
use stele_common::models::{Memory, MemoryType};
use stele_common::query::SearchParams;

#[derive(Clone)]
pub struct ApiState {
    pub db: DbPool,
    #[cfg(feature = "stylos")]
    pub stylos: Option<std::sync::Arc<crate::stylos_module::StylosStatusSource>>,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/health", get(get_health))
        .route("/v1/memories", get(list_memories).post(create_memory))
        .route(
            "/v1/memories/{id}",
            get(get_memory).put(update_memory).delete(delete_memory),
        )
        .route("/v1/scopes", get(list_scopes))
        .route("/v1/tags", get(list_tags))
        .route("/v1/stats", get(get_stats))
        // Knowledge Graph routes
        .route("/v1/graph", get(graph_read))
        .route(
            "/v1/graph/entities",
            get(graph_search_entities).post(graph_create_entities),
        )
        .route(
            "/v1/graph/entities/{name}",
            get(graph_get_entity).delete(graph_delete_entity),
        )
        .route(
            "/v1/graph/entities/{name}/observations",
            axum::routing::post(graph_add_observations).delete(graph_delete_observations),
        )
        .route(
            "/v1/graph/relations",
            axum::routing::post(graph_create_relations).delete(graph_delete_relations),
        )
        .route("/v1/graph/open", get(graph_open_nodes))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

// --- Request / Response types ---

#[derive(Deserialize)]
struct CreateMemoryRequest {
    title: String,
    content: String,
    scope: String,
    #[serde(default)]
    tags: Vec<String>,
    memory_type: Option<String>,
    author: Option<String>,
}

#[derive(Deserialize)]
struct UpdateMemoryRequest {
    title: Option<String>,
    content: Option<String>,
    scope: Option<String>,
    tags: Option<Vec<String>>,
    memory_type: Option<String>,
}

#[derive(Deserialize)]
struct MemoriesQuery {
    q: Option<String>,
    scope: Option<String>,
    tags: Option<String>,
    #[serde(default)]
    match_all_tags: bool,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ScopesQuery {
    prefix: Option<String>,
}

#[derive(Deserialize)]
struct TagsQuery {
    scope: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct DeleteResponse {
    deleted: bool,
    id: String,
}

#[derive(Serialize)]
struct StatsResponse {
    total_memories: i64,
    total_scopes: i64,
    total_tags: i64,
    recent_memories: Vec<RecentMemory>,
}

#[derive(Serialize)]
struct RecentMemory {
    id: String,
    title: String,
    scope: String,
    updated_at: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    db_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    stylos: Option<serde_json::Value>,
}

// --- Handlers ---

async fn get_health(State(state): State<ApiState>) -> impl IntoResponse {
    let db_ok = {
        let conn = state.db.lock().await;
        conn.execute_batch("SELECT 1").is_ok()
    };
    #[cfg(feature = "stylos")]
    let stylos = match &state.stylos {
        Some(s) => Some(serde_json::to_value(s.to_health().await).unwrap()),
        None => Some(serde_json::json!({ "enabled": false })),
    };
    #[cfg(not(feature = "stylos"))]
    let stylos: Option<serde_json::Value> = None;
    Json(
        serde_json::to_value(HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
            db_ok,
            stylos,
        })
        .unwrap(),
    )
}

async fn create_memory(
    State(state): State<ApiState>,
    Json(req): Json<CreateMemoryRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let id = ulid::Ulid::new().to_string();

    let memory = Memory {
        id,
        title: req.title,
        content: req.content,
        memory_type: MemoryType::from_str(req.memory_type.as_deref().unwrap_or("knowledge")),
        scope: req.scope,
        author: req.author,
        tags: req.tags,
        created_at: now.clone(),
        updated_at: now,
    };

    let conn = db.lock().await;
    match db::insert_memory(&conn, &memory) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(&memory).unwrap()),
        )
            .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_memories(
    State(state): State<ApiState>,
    Query(q): Query<MemoriesQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let tags = q.tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });
    let scopes = q.scope.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    });

    let search = SearchParams {
        query: q.q,
        scope: scopes,
        tags,
        match_all_tags: q.match_all_tags,
        limit: q.limit,
    };

    let conn = db.lock().await;
    match db::search_memories(&conn, &search) {
        Ok(results) => Json(serde_json::to_value(&results).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_memory(State(state): State<ApiState>, Path(id): Path<String>) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    match db::get_memory(&conn, &id) {
        Ok(Some(m)) => Json(serde_json::to_value(&m).unwrap()).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn update_memory(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMemoryRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    match db::update_memory(
        &conn,
        &id,
        req.title.as_deref(),
        req.content.as_deref(),
        req.scope.as_deref(),
        req.tags.as_deref(),
        req.memory_type.as_deref(),
    ) {
        Ok(true) => match db::get_memory(&conn, &id) {
            Ok(Some(m)) => Json(serde_json::to_value(&m).unwrap()).into_response(),
            Ok(None) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn delete_memory(State(state): State<ApiState>, Path(id): Path<String>) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    match db::delete_memory(&conn, &id) {
        Ok(true) => Json(serde_json::to_value(&DeleteResponse { deleted: true, id }).unwrap())
            .into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_scopes(
    State(state): State<ApiState>,
    Query(q): Query<ScopesQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    match db::list_scopes(&conn, q.prefix.as_deref()) {
        Ok(scopes) => Json(serde_json::to_value(&scopes).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_tags(State(state): State<ApiState>, Query(q): Query<TagsQuery>) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scopes: Option<Vec<String>> = q.scope.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    });
    match db::list_tags(&conn, scopes.as_deref()) {
        Ok(tags) => Json(serde_json::to_value(&tags).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_stats(State(state): State<ApiState>) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    match db::get_stats(&conn) {
        Ok(stats) => {
            let recent: Vec<RecentMemory> = stats
                .recent_memories
                .into_iter()
                .map(|m| RecentMemory {
                    id: m.id,
                    title: m.title,
                    scope: m.scope,
                    updated_at: m.updated_at,
                })
                .collect();

            Json(
                serde_json::to_value(&StatsResponse {
                    total_memories: stats.total_memories,
                    total_scopes: stats.total_scopes,
                    total_tags: stats.total_tags,
                    recent_memories: recent,
                })
                .unwrap(),
            )
            .into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (
        status,
        Json(
            serde_json::to_value(&ErrorResponse {
                error: message.to_string(),
            })
            .unwrap(),
        ),
    )
        .into_response()
}

// ── Knowledge Graph request types ──

#[derive(Deserialize)]
struct EntityInput {
    name: String,
    entity_type: String,
    #[serde(default)]
    observations: Vec<String>,
}

#[derive(Deserialize)]
struct CreateEntitiesRequest {
    entities: Vec<EntityInput>,
    scope: String,
}

#[derive(Deserialize)]
struct RelationInput {
    from: String,
    to: String,
    relation_type: String,
}

#[derive(Deserialize)]
struct CreateRelationsRequest {
    relations: Vec<RelationInput>,
    scope: String,
}

#[derive(Deserialize)]
struct ObservationsRequest {
    observations: Vec<String>,
}

#[derive(Deserialize)]
struct DeleteRelationsRequest {
    relations: Vec<RelationInput>,
    scope: String,
}

#[derive(Deserialize)]
struct GraphQuery {
    scope: Option<String>,
}

#[derive(Deserialize)]
struct EntityQuery {
    q: Option<String>,
    scope: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct EntityScopeQuery {
    scope: Option<String>,
}

#[derive(Deserialize)]
struct OpenNodesQuery {
    names: String,
    scope: Option<String>,
}

// ── Knowledge Graph handlers ──

async fn graph_create_entities(
    State(state): State<ApiState>,
    Json(req): Json<CreateEntitiesRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let mut results = Vec::new();

    for input in &req.entities {
        match db::insert_entity(
            &conn,
            &input.name,
            &input.entity_type,
            &req.scope,
            &input.observations,
        ) {
            Ok((id, created)) => results.push(serde_json::json!({
                "name": input.name,
                "id": id,
                "created": created,
            })),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::to_value(&results).unwrap()),
    )
        .into_response()
}

async fn graph_search_entities(
    State(state): State<ApiState>,
    Query(q): Query<EntityQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let limit = q.limit.unwrap_or(20).min(100);

    let scopes: Option<Vec<String>> = q.scope.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    });

    if let Some(query) = &q.q {
        match db::search_entities(&conn, query, scopes.as_deref(), limit) {
            Ok(results) => Json(serde_json::to_value(&results).unwrap()).into_response(),
            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    } else {
        // No query — return full graph for scope
        let scope_vec = scopes.unwrap_or_else(|| vec![String::new()]);
        match db::read_graph(&conn, &scope_vec) {
            Ok(graph) => Json(serde_json::to_value(&graph.entities).unwrap()).into_response(),
            Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    }
}

async fn graph_get_entity(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    Query(q): Query<EntityScopeQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scope = q.scope.as_deref().unwrap_or("");
    match db::get_entity_by_name(&conn, &name, scope) {
        Ok(Some(entity)) => Json(serde_json::to_value(&entity).unwrap()).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Entity not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn graph_delete_entity(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    Query(q): Query<EntityScopeQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scope = q.scope.as_deref().unwrap_or("");
    match db::delete_entity(&conn, &name, scope) {
        Ok(true) => Json(
            serde_json::to_value(&DeleteResponse {
                deleted: true,
                id: name,
            })
            .unwrap(),
        )
        .into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Entity not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn graph_add_observations(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    Query(q): Query<EntityScopeQuery>,
    Json(req): Json<ObservationsRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scope = q.scope.as_deref().unwrap_or("");
    match db::insert_observations(&conn, &name, scope, &req.observations) {
        Ok(Some(entity)) => Json(serde_json::to_value(&entity).unwrap()).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Entity not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn graph_delete_observations(
    State(state): State<ApiState>,
    Path(name): Path<String>,
    Query(q): Query<EntityScopeQuery>,
    Json(req): Json<ObservationsRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scope = q.scope.as_deref().unwrap_or("");
    match db::delete_observations(&conn, &name, scope, &req.observations) {
        Ok(Some(entity)) => Json(serde_json::to_value(&entity).unwrap()).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Entity not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn graph_create_relations(
    State(state): State<ApiState>,
    Json(req): Json<CreateRelationsRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let mut results = Vec::new();

    for input in &req.relations {
        match db::insert_relation(
            &conn,
            &input.from,
            &input.to,
            &input.relation_type,
            &req.scope,
        ) {
            Ok(Some((id, created))) => results.push(serde_json::json!({
                "from": input.from,
                "to": input.to,
                "relation_type": input.relation_type,
                "id": id,
                "created": created,
            })),
            Ok(None) => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    &format!(
                        "Entity '{}' or '{}' not found in scope '{}'",
                        input.from, input.to, req.scope
                    ),
                )
            }
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    }

    (
        StatusCode::CREATED,
        Json(serde_json::to_value(&results).unwrap()),
    )
        .into_response()
}

async fn graph_delete_relations(
    State(state): State<ApiState>,
    Json(req): Json<DeleteRelationsRequest>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let mut deleted = Vec::new();
    let mut not_found = Vec::new();

    for input in &req.relations {
        match db::delete_relation(
            &conn,
            &input.from,
            &input.to,
            &input.relation_type,
            &req.scope,
        ) {
            Ok(true) => deleted.push(format!(
                "{} --[{}]--> {}",
                input.from, input.relation_type, input.to
            )),
            Ok(false) => not_found.push(format!(
                "{} --[{}]--> {}",
                input.from, input.relation_type, input.to
            )),
            Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        }
    }

    Json(
        serde_json::to_value(serde_json::json!({
            "deleted": deleted,
            "not_found": not_found,
        }))
        .unwrap(),
    )
    .into_response()
}

async fn graph_read(State(state): State<ApiState>, Query(q): Query<GraphQuery>) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scopes: Vec<String> = q
        .scope
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec![String::new()]);
    match db::read_graph(&conn, &scopes) {
        Ok(graph) => Json(serde_json::to_value(&graph).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn graph_open_nodes(
    State(state): State<ApiState>,
    Query(q): Query<OpenNodesQuery>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let conn = db.lock().await;
    let scopes: Vec<String> = q
        .scope
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec![String::new()]);
    let names: Vec<String> = q
        .names
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match db::open_entities(&conn, &names, &scopes) {
        Ok(graph) => Json(serde_json::to_value(&graph).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
