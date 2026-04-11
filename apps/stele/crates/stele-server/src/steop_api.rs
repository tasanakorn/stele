use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::db::{self, DbPool};

pub fn router(db: DbPool) -> Router {
    Router::new()
        .route("/steop.session.start", post(session_start))
        .route("/steop.session.stop", post(session_stop))
        .route("/steop.session.touch", post(session_touch))
        .route("/steop.session.get", post(session_get))
        .route("/steop.session.list", post(session_list))
        .route("/steop.project.list", post(project_list))
        .route("/steop.state.get", post(state_get))
        .route("/steop.state.put", post(state_put))
        .route("/steop.state.incr", post(state_incr))
        .route("/steop.state.reset", post(state_reset))
        .route("/steop.state.delete", post(state_delete))
        .route("/steop.status.get", post(status_get))
        .route("/steop.storage.put", post(storage_put))
        .route("/steop.storage.get", post(storage_get))
        .route("/steop.storage.delete", post(storage_delete))
        .route("/steop.storage.list", post(storage_list))
        .route("/steop.log.append", post(log_append))
        .route("/steop.log.query", post(log_query))
        .route("/steop.mailbox.send", post(mailbox_send))
        .route("/steop.mailbox.list", post(mailbox_list))
        .route("/steop.mailbox.ack", post(mailbox_ack))
        .route("/steop.notify", post(notify_handler))
        .with_state(db)
        .layer(CorsLayer::permissive())
}

fn err500(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": e.to_string() })),
    )
        .into_response()
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response()
}

fn default_true() -> bool {
    true
}

// ── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SessionStartReq {
    host: String,
    project_dir: String,
    session_id: String,
    data: Option<Value>,
}

#[derive(Deserialize)]
struct SessionRef {
    host: String,
    project_dir: String,
    session_id: String,
}

#[derive(Deserialize)]
struct ShortOrFullRef {
    host: Option<String>,
    project_dir: Option<String>,
    session_id: String,
}

#[derive(Deserialize)]
struct SessionListReq {
    host: Option<String>,
    project_dir: Option<String>,
    state: Option<String>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct ProjectListReq {
    host: Option<String>,
}

#[derive(Deserialize)]
struct StatePutReq {
    host: String,
    project_dir: String,
    session_id: String,
    data: Value,
    #[serde(default = "default_true")]
    merge: bool,
}

#[derive(Deserialize)]
struct CounterReq {
    host: String,
    project_dir: String,
    session_id: String,
    counter: String,
    #[serde(default)]
    delta: Option<i64>,
    #[serde(default)]
    value: Option<i64>,
}

#[derive(Deserialize)]
struct StorageReq {
    host: String,
    project_dir: String,
    session_id: Option<String>,
    key: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct LogAppendReq {
    host: String,
    project_dir: String,
    session_id: String,
    event: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Deserialize)]
struct LogQueryReq {
    host: Option<String>,
    project_dir: Option<String>,
    session_id: Option<String>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct MailboxSendReq {
    from_host: String,
    from_project_dir: String,
    from_session_id: String,
    to_host: String,
    to_project_dir: String,
    to_session_id: Option<String>,
    payload: Value,
}

#[derive(Deserialize)]
struct MailboxListReq {
    to_host: String,
    to_project_dir: String,
    to_session_id: Option<String>,
    limit: Option<i64>,
    #[serde(default)]
    include_acked: bool,
}

#[derive(Deserialize)]
struct MailboxAckReq {
    id: i64,
}

#[derive(Deserialize)]
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
struct NotifyReq {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    sound: bool,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn session_start(
    State(db): State<DbPool>,
    Json(req): Json<SessionStartReq>,
) -> Response {
    let conn = db.lock().await;
    match db::steop_session_start(&conn, &req.host, &req.project_dir, &req.session_id, req.data) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err500(e),
    }
}

async fn session_stop(State(db): State<DbPool>, Json(req): Json<SessionRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_session_stop(&conn, &req.host, &req.project_dir, &req.session_id) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn session_touch(State(db): State<DbPool>, Json(req): Json<SessionRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_session_touch(&conn, &req.host, &req.project_dir, &req.session_id) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn session_get(State(db): State<DbPool>, Json(req): Json<ShortOrFullRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_session_get(
        &conn,
        req.host.as_deref(),
        req.project_dir.as_deref(),
        &req.session_id,
    ) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn session_list(State(db): State<DbPool>, Json(req): Json<SessionListReq>) -> Response {
    let conn = db.lock().await;
    let limit = req.limit.unwrap_or(100);
    match db::steop_session_list(
        &conn,
        req.host.as_deref(),
        req.project_dir.as_deref(),
        req.state.as_deref(),
        limit,
    ) {
        Ok(sessions) => Json(json!({ "sessions": sessions })).into_response(),
        Err(e) => err500(e),
    }
}

async fn project_list(State(db): State<DbPool>, Json(req): Json<ProjectListReq>) -> Response {
    let conn = db.lock().await;
    match db::steop_project_list(&conn, req.host.as_deref()) {
        Ok(pairs) => {
            let projects: Vec<_> = pairs
                .into_iter()
                .map(|(h, pd)| json!({ "host": h, "project_dir": pd }))
                .collect();
            Json(json!({ "projects": projects })).into_response()
        }
        Err(e) => err500(e),
    }
}

async fn state_get(State(db): State<DbPool>, Json(req): Json<ShortOrFullRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_session_get(
        &conn,
        req.host.as_deref(),
        req.project_dir.as_deref(),
        &req.session_id,
    ) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn state_put(State(db): State<DbPool>, Json(req): Json<StatePutReq>) -> Response {
    let conn = db.lock().await;
    match db::steop_state_put(
        &conn,
        &req.host,
        &req.project_dir,
        &req.session_id,
        req.data,
        req.merge,
    ) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_incr(State(db): State<DbPool>, Json(req): Json<CounterReq>) -> Response {
    let conn = db.lock().await;
    let delta = req.delta.unwrap_or(1);
    match db::steop_state_incr(
        &conn,
        &req.host,
        &req.project_dir,
        &req.session_id,
        &req.counter,
        delta,
    ) {
        Ok(v) => Json(json!({ "counter": req.counter, "value": v })).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_reset(State(db): State<DbPool>, Json(req): Json<CounterReq>) -> Response {
    let conn = db.lock().await;
    let value = req.value.unwrap_or(0);
    match db::steop_state_reset(
        &conn,
        &req.host,
        &req.project_dir,
        &req.session_id,
        &req.counter,
        value,
    ) {
        Ok(v) => Json(json!({ "counter": req.counter, "value": v })).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_delete(State(db): State<DbPool>, Json(req): Json<SessionRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_state_delete(&conn, &req.host, &req.project_dir, &req.session_id) {
        Ok(deleted) => Json(json!({ "deleted": deleted })).into_response(),
        Err(e) => err500(e),
    }
}

async fn status_get(State(db): State<DbPool>, Json(req): Json<ShortOrFullRef>) -> Response {
    let conn = db.lock().await;
    match db::steop_status_get(
        &conn,
        req.host.as_deref(),
        req.project_dir.as_deref(),
        &req.session_id,
    ) {
        Ok(p) => Json(p).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_put(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let key = req.key.as_deref().unwrap_or("");
    let content = req.content.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = match req.session_id.as_deref().filter(|s| !s.is_empty()) {
        Some(sid) => {
            db::steop_storage_session_put(&conn, &req.host, &req.project_dir, sid, key, content)
        }
        None => db::steop_storage_project_put(&conn, &req.host, &req.project_dir, key, content),
    };
    match res {
        Ok(m) => Json(m).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_get(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let key = req.key.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = match req.session_id.as_deref().filter(|s| !s.is_empty()) {
        Some(sid) => db::steop_storage_session_get(&conn, &req.host, &req.project_dir, sid, key),
        None => db::steop_storage_project_get(&conn, &req.host, &req.project_dir, key),
    };
    match res {
        Ok(Some(b)) => Json(b).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn storage_delete(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let key = req.key.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = match req.session_id.as_deref().filter(|s| !s.is_empty()) {
        Some(sid) => {
            db::steop_storage_session_delete(&conn, &req.host, &req.project_dir, sid, key)
        }
        None => db::steop_storage_project_delete(&conn, &req.host, &req.project_dir, key),
    };
    match res {
        Ok(deleted) => Json(json!({ "deleted": deleted })).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_list(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let conn = db.lock().await;
    let res = match req.session_id.as_deref().filter(|s| !s.is_empty()) {
        Some(sid) => db::steop_storage_session_list(&conn, &req.host, &req.project_dir, sid),
        None => db::steop_storage_project_list(&conn, &req.host, &req.project_dir),
    };
    match res {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(e) => err500(e),
    }
}

async fn log_append(State(db): State<DbPool>, Json(req): Json<LogAppendReq>) -> Response {
    let conn = db.lock().await;
    let default_data = Value::Object(Default::default());
    let data = req.data.as_ref().unwrap_or(&default_data);
    match db::steop_log_append(
        &conn,
        &req.host,
        &req.project_dir,
        &req.session_id,
        &req.event,
        data,
    ) {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(e) => err500(e),
    }
}

async fn log_query(State(db): State<DbPool>, Json(req): Json<LogQueryReq>) -> Response {
    let conn = db.lock().await;
    let limit = req.limit.unwrap_or(200);
    match db::steop_log_query(
        &conn,
        req.host.as_deref(),
        req.project_dir.as_deref(),
        req.session_id.as_deref(),
        limit,
    ) {
        Ok(logs) => Json(json!({ "logs": logs })).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_send(State(db): State<DbPool>, Json(req): Json<MailboxSendReq>) -> Response {
    let conn = db.lock().await;
    let to_sid = req.to_session_id.as_deref().unwrap_or("");
    match db::steop_mailbox_send(
        &conn,
        &req.from_host,
        &req.from_project_dir,
        &req.from_session_id,
        &req.to_host,
        &req.to_project_dir,
        to_sid,
        &req.payload,
    ) {
        Ok(id) => Json(json!({ "id": id })).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_list(State(db): State<DbPool>, Json(req): Json<MailboxListReq>) -> Response {
    let conn = db.lock().await;
    let to_sid = req.to_session_id.as_deref().unwrap_or("");
    let limit = req.limit.unwrap_or(200);
    match db::steop_mailbox_list(
        &conn,
        &req.to_host,
        &req.to_project_dir,
        to_sid,
        limit,
        req.include_acked,
    ) {
        Ok(messages) => Json(json!({ "messages": messages })).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_ack(State(db): State<DbPool>, Json(req): Json<MailboxAckReq>) -> Response {
    let conn = db.lock().await;
    match db::steop_mailbox_ack(&conn, req.id) {
        Ok(acked) => Json(json!({ "acked": acked })).into_response(),
        Err(e) => err500(e),
    }
}

async fn notify_handler(State(_db): State<DbPool>, Json(req): Json<NotifyReq>) -> Response {
    #[cfg(feature = "desktop")]
    {
        let notification = crate::notify::NotificationRequest {
            title: req.title.unwrap_or_else(|| "Steop".to_string()),
            body: req.body.unwrap_or_default(),
            subtitle: req.subtitle,
            sound: req.sound,
        };
        match crate::notify::show(&notification) {
            Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
            Err(e) => err500(e),
        }
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = req;
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": "notify unavailable in headless mode" })),
        )
            .into_response()
    }
}
