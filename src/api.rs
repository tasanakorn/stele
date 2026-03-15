use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

use crate::db::{self, DbPool};
use crate::models::{Memory, MemoryType};
use crate::query::SearchParams;

pub fn router(db: DbPool) -> Router {
    Router::new()
        .route("/v1/memories", get(list_memories).post(create_memory))
        .route(
            "/v1/memories/{id}",
            get(get_memory).put(update_memory).delete(delete_memory),
        )
        .route("/v1/scopes", get(list_scopes))
        .route("/v1/tags", get(list_tags))
        .route("/v1/stats", get(get_stats))
        .layer(CorsLayer::permissive())
        .with_state(db)
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

// --- Handlers ---

async fn create_memory(
    State(db): State<DbPool>,
    Json(req): Json<CreateMemoryRequest>,
) -> impl IntoResponse {
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
        Ok(()) => (StatusCode::CREATED, Json(serde_json::to_value(&memory).unwrap())).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_memories(
    State(db): State<DbPool>,
    Query(q): Query<MemoriesQuery>,
) -> impl IntoResponse {
    let tags = q.tags.map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());

    let search = SearchParams {
        query: q.q,
        scope: q.scope,
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

async fn get_memory(
    State(db): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::get_memory(&conn, &id) {
        Ok(Some(m)) => Json(serde_json::to_value(&m).unwrap()).into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn update_memory(
    State(db): State<DbPool>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMemoryRequest>,
) -> impl IntoResponse {
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

async fn delete_memory(
    State(db): State<DbPool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::delete_memory(&conn, &id) {
        Ok(true) => Json(serde_json::to_value(&DeleteResponse { deleted: true, id }).unwrap()).into_response(),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "Memory not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_scopes(
    State(db): State<DbPool>,
    Query(q): Query<ScopesQuery>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::list_scopes(&conn, q.prefix.as_deref()) {
        Ok(scopes) => Json(serde_json::to_value(&scopes).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn list_tags(
    State(db): State<DbPool>,
    Query(q): Query<TagsQuery>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::list_tags(&conn, q.scope.as_deref()) {
        Ok(tags) => Json(serde_json::to_value(&tags).unwrap()).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_stats(State(db): State<DbPool>) -> impl IntoResponse {
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

            Json(serde_json::to_value(&StatsResponse {
                total_memories: stats.total_memories,
                total_scopes: stats.total_scopes,
                total_tags: stats.total_tags,
                recent_memories: recent,
            }).unwrap()).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::to_value(&ErrorResponse { error: message.to_string() }).unwrap())).into_response()
}
