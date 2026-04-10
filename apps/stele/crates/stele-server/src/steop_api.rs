use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tower_http::cors::CorsLayer;

use crate::db::{self, DbPool};

pub fn router(db: DbPool) -> Router {
    Router::new()
        .route(
            "/storage",
            put(storage_put).get(storage_get).delete(storage_delete),
        )
        .route("/storage/list", get(storage_list))
        .route(
            "/state/{session_id}",
            get(state_get).put(state_put).delete(state_delete),
        )
        .route("/state/{session_id}/incr", post(counter_incr))
        .route("/state/{session_id}/reset", post(counter_reset))
        .route("/status/{session_id}", get(status_get))
        .route("/notify", post(notify_handler))
        .layer(CorsLayer::permissive())
        .with_state(db)
}

// ── Common helpers ──

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
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

// ── Storage ──

#[derive(Deserialize)]
struct StorageQuery {
    scope: String,
    key: String,
}

#[derive(Deserialize)]
struct StorageListQuery {
    scope: String,
}

#[derive(Deserialize)]
struct StoragePutBody {
    content: String,
}

#[derive(Serialize)]
struct StorageBlobResponse {
    scope: String,
    key: String,
    content: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct StoragePutResponse {
    scope: String,
    key: String,
    updated_at: String,
}

#[derive(Serialize)]
struct StorageListItem {
    key: String,
    updated_at: String,
    size: i64,
}

#[derive(Serialize)]
struct StorageListResponse {
    scope: String,
    items: Vec<StorageListItem>,
}

#[derive(Serialize)]
struct StorageDeleteResponse {
    deleted: bool,
    scope: String,
    key: String,
}

async fn storage_put(
    State(db): State<DbPool>,
    Query(q): Query<StorageQuery>,
    Json(body): Json<StoragePutBody>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_storage_put(&conn, &q.scope, &q.key, &body.content) {
        Ok(meta) => Json(
            serde_json::to_value(&StoragePutResponse {
                scope: meta.scope,
                key: meta.key,
                updated_at: meta.updated_at,
            })
            .unwrap(),
        )
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn storage_get(State(db): State<DbPool>, Query(q): Query<StorageQuery>) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_storage_get(&conn, &q.scope, &q.key) {
        Ok(Some(blob)) => Json(
            serde_json::to_value(&StorageBlobResponse {
                scope: blob.scope,
                key: blob.key,
                content: blob.content,
                created_at: blob.created_at,
                updated_at: blob.updated_at,
            })
            .unwrap(),
        )
        .into_response(),
        Ok(None) => error_response(StatusCode::NOT_FOUND, "not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn storage_delete(
    State(db): State<DbPool>,
    Query(q): Query<StorageQuery>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_storage_delete(&conn, &q.scope, &q.key) {
        Ok(deleted) => Json(
            serde_json::to_value(&StorageDeleteResponse {
                deleted,
                scope: q.scope,
                key: q.key,
            })
            .unwrap(),
        )
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn storage_list(
    State(db): State<DbPool>,
    Query(q): Query<StorageListQuery>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_storage_list(&conn, &q.scope) {
        Ok(items) => {
            let items: Vec<StorageListItem> = items
                .into_iter()
                .map(|i| StorageListItem {
                    key: i.key,
                    updated_at: i.updated_at,
                    size: i.size,
                })
                .collect();
            Json(
                serde_json::to_value(&StorageListResponse {
                    scope: q.scope,
                    items,
                })
                .unwrap(),
            )
            .into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── State ──

#[derive(Serialize)]
struct StateResponse {
    session_id: String,
    data: serde_json::Value,
    counters: BTreeMap<String, i64>,
    created_at: String,
    updated_at: String,
}

impl From<db::SteopState> for StateResponse {
    fn from(s: db::SteopState) -> Self {
        StateResponse {
            session_id: s.session_id,
            data: s.data,
            counters: s.counters,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct StatePutBody {
    #[serde(default)]
    data: serde_json::Value,
    #[serde(default = "default_merge")]
    merge: bool,
}

fn default_merge() -> bool {
    true
}

#[derive(Deserialize)]
struct CounterIncrBody {
    counter: String,
    #[serde(default = "default_delta")]
    delta: i64,
}

fn default_delta() -> i64 {
    1
}

#[derive(Deserialize)]
struct CounterResetBody {
    counter: String,
    #[serde(default)]
    value: i64,
}

#[derive(Serialize)]
struct CounterResponse {
    counter: String,
    value: i64,
}

#[derive(Serialize)]
struct StateDeleteResponse {
    deleted: bool,
    session_id: String,
}

async fn state_get(State(db): State<DbPool>, Path(session_id): Path<String>) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_state_get(&conn, &session_id) {
        Ok(Some(state)) => {
            Json(serde_json::to_value(StateResponse::from(state)).unwrap()).into_response()
        }
        Ok(None) => error_response(StatusCode::NOT_FOUND, "not found"),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn state_put(
    State(db): State<DbPool>,
    Path(session_id): Path<String>,
    Json(body): Json<StatePutBody>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    let data = if body.data.is_null() {
        serde_json::Value::Object(Default::default())
    } else {
        body.data
    };
    match db::steop_state_put(&conn, &session_id, data, body.merge) {
        Ok(state) => {
            Json(serde_json::to_value(StateResponse::from(state)).unwrap()).into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn state_delete(
    State(db): State<DbPool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_state_delete(&conn, &session_id) {
        Ok(deleted) => Json(
            serde_json::to_value(&StateDeleteResponse {
                deleted,
                session_id,
            })
            .unwrap(),
        )
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn counter_incr(
    State(db): State<DbPool>,
    Path(session_id): Path<String>,
    Json(body): Json<CounterIncrBody>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_counter_incr(&conn, &session_id, &body.counter, body.delta) {
        Ok(value) => Json(
            serde_json::to_value(&CounterResponse {
                counter: body.counter,
                value,
            })
            .unwrap(),
        )
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn counter_reset(
    State(db): State<DbPool>,
    Path(session_id): Path<String>,
    Json(body): Json<CounterResetBody>,
) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_counter_reset(&conn, &session_id, &body.counter, body.value) {
        Ok(value) => Json(
            serde_json::to_value(&CounterResponse {
                counter: body.counter,
                value,
            })
            .unwrap(),
        )
        .into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

// ── Notify ──

#[derive(Deserialize, Default)]
struct NotifyRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    sound: bool,
}

async fn notify_handler(Json(req): Json<NotifyRequest>) -> impl IntoResponse {
    let notification = crate::notify::NotificationRequest {
        title: req.title.unwrap_or_else(|| "Claude Code".to_string()),
        body: req
            .body
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Session finished".to_string()),
        subtitle: req.subtitle,
        sound: req.sound,
    };
    match crate::notify::show(&notification) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(msg) if msg.contains("unavailable") => (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(msg) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &msg),
    }
}

// ── Status (HUD) ──

#[derive(Serialize)]
struct StatusResponse {
    session_id: String,
    mode: String,
    phase: String,
    step: String,
    tool_calls: i64,
    loop_count: i64,
    step_retry: i64,
    updated_at: String,
}

fn json_string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn json_i64_field(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| v.as_i64())
}

async fn status_get(State(db): State<DbPool>, Path(session_id): Path<String>) -> impl IntoResponse {
    let conn = db.lock().await;
    match db::steop_state_get(&conn, &session_id) {
        Ok(Some(state)) => {
            let mode = {
                let m = json_string_field(&state.data, "mode");
                if m.is_empty() {
                    "idle".to_string()
                } else {
                    m
                }
            };
            let phase = json_string_field(&state.data, "phase");
            let step = match (
                json_i64_field(&state.data, "current_step"),
                json_i64_field(&state.data, "total_steps"),
            ) {
                (Some(cur), Some(total)) => format!("{}/{}", cur, total),
                _ => "-".to_string(),
            };
            let tool_calls = state.counters.get("tool_calls").copied().unwrap_or(0);
            let loop_count = state.counters.get("loop_count").copied().unwrap_or(0);
            let step_retry = state.counters.get("step_retry").copied().unwrap_or(0);
            Json(
                serde_json::to_value(&StatusResponse {
                    session_id: state.session_id,
                    mode,
                    phase,
                    step,
                    tool_calls,
                    loop_count,
                    step_retry,
                    updated_at: state.updated_at,
                })
                .unwrap(),
            )
            .into_response()
        }
        Ok(None) => {
            let now = chrono::Utc::now().to_rfc3339();
            Json(
                serde_json::to_value(&StatusResponse {
                    session_id,
                    mode: "idle".to_string(),
                    phase: String::new(),
                    step: "-".to_string(),
                    tool_calls: 0,
                    loop_count: 0,
                    step_retry: 0,
                    updated_at: now,
                })
                .unwrap(),
            )
            .into_response()
        }
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
