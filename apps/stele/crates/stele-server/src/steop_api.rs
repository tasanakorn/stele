use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;

use crate::db::{self, DbPool};
use crate::serde_helpers::string_or_string_vec_opt;

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
        .route("/steop.mailbox.get", post(mailbox_get))
        .route("/steop.mailbox.read", post(mailbox_read))
        .route("/steop.mailbox.archive", post(mailbox_archive))
        .route("/steop.mailbox.update_meta", post(mailbox_update_meta))
        .route("/steop.notify", post(notify_handler))
        .with_state(db)
        .layer(CorsLayer::permissive())
}

// ── Composite ID parsing ─────────────────────────────────────────────────────

static UUID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
enum Principal {
    Project,
    Session(String),
    User,
}

#[derive(Debug, Clone)]
struct ParsedId {
    host: String,
    project_dir: String,
    principal: Principal,
}

impl ParsedId {
    fn session_id(&self) -> &str {
        match &self.principal {
            Principal::Session(s) => s.as_str(),
            _ => "",
        }
    }
    fn is_session(&self) -> bool {
        matches!(self.principal, Principal::Session(_))
    }
    fn is_project(&self) -> bool {
        matches!(self.principal, Principal::Project)
    }
}

const USER_LITERAL: &str = "USER";

fn parse_id(id: &str) -> Result<ParsedId, String> {
    let (host, rest) = id
        .split_once(':')
        .ok_or_else(|| "id missing ':' separator".to_string())?;
    if host.is_empty() {
        return Err("id host segment is empty".into());
    }
    if rest.is_empty() {
        return Err("id project_dir segment is empty".into());
    }
    if let Some(idx) = rest.rfind(':') {
        let tail = &rest[idx + 1..];
        let pd = &rest[..idx];
        if pd.is_empty() {
            return Err("id project_dir segment is empty".into());
        }
        if tail.is_empty() {
            return Err("id 3rd segment is empty".into());
        }
        if UUID_RE.is_match(tail) {
            return Ok(ParsedId {
                host: host.to_string(),
                project_dir: pd.to_string(),
                principal: Principal::Session(tail.to_string()),
            });
        }
        if tail == USER_LITERAL {
            return Ok(ParsedId {
                host: host.to_string(),
                project_dir: pd.to_string(),
                principal: Principal::User,
            });
        }
        return Err(
            "id 3rd segment must be a session UUID or the literal 'USER'".into(),
        );
    }
    Ok(ParsedId {
        host: host.to_string(),
        project_dir: rest.to_string(),
        principal: Principal::Project,
    })
}

/// Parse an id that MUST be a 3-segment session form (rejects project and user forms).
fn parse_full_id(id: &str) -> Result<ParsedId, String> {
    let p = parse_id(id)?;
    if !p.is_session() {
        return Err("id must be 3-segment (host:project_dir:session_uuid)".into());
    }
    Ok(p)
}

fn err400(msg: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": msg.to_string() })),
    )
        .into_response()
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

fn err409(msg: impl std::fmt::Display) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({ "error": msg.to_string() })),
    )
        .into_response()
}

fn default_true() -> bool {
    true
}

// ── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

#[derive(Deserialize)]
struct SessionStartReq {
    id: String,
    data: Option<Value>,
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
    id: String,
    data: Value,
    #[serde(default = "default_true")]
    merge: bool,
}

#[derive(Deserialize)]
struct CounterReq {
    id: String,
    counter: String,
    #[serde(default)]
    delta: Option<i64>,
    #[serde(default)]
    value: Option<i64>,
}

#[derive(Deserialize)]
struct StorageReq {
    id: String,
    key: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct LogAppendReq {
    id: String,
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
    id: String,
    to: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    message_type: Option<String>,
    #[serde(default)]
    meta: Option<Value>,
    #[serde(default)]
    payload: Option<Value>,
}

#[derive(Deserialize)]
struct MailboxListReq {
    id: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default, deserialize_with = "string_or_string_vec_opt")]
    status: Option<Vec<String>>,
    #[serde(default)]
    message_type: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct MailboxRowReq {
    #[allow(dead_code)]
    id: String,
    message_id: i64,
}

#[derive(Deserialize)]
struct MailboxUpdateMetaReq {
    id: String,
    message_id: i64,
    #[serde(default)]
    meta_patch: Value,
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
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_session_start(&conn, &p.host, &p.project_dir, p.session_id(), req.data) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err500(e),
    }
}

async fn session_stop(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_session_stop(&conn, &p.host, &p.project_dir, p.session_id()) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn session_touch(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_session_touch(&conn, &p.host, &p.project_dir, p.session_id()) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn session_get(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_session_get(&conn, &p.host, &p.project_dir, p.session_id()) {
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
                .map(|(h, pd)| json!({ "id": format!("{}:{}", h, pd) }))
                .collect();
            Json(json!({ "projects": projects })).into_response()
        }
        Err(e) => err500(e),
    }
}

async fn state_get(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_session_get(&conn, &p.host, &p.project_dir, p.session_id()) {
        Ok(Some(s)) => Json(s).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn state_put(State(db): State<DbPool>, Json(req): Json<StatePutReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_state_put(
        &conn,
        &p.host,
        &p.project_dir,
        p.session_id(),
        req.data,
        req.merge,
    ) {
        Ok(s) => Json(s).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_incr(State(db): State<DbPool>, Json(req): Json<CounterReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    let delta = req.delta.unwrap_or(1);
    match db::steop_state_incr(
        &conn,
        &p.host,
        &p.project_dir,
        p.session_id(),
        &req.counter,
        delta,
    ) {
        Ok(v) => Json(json!({ "counter": req.counter, "value": v })).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_reset(State(db): State<DbPool>, Json(req): Json<CounterReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    let value = req.value.unwrap_or(0);
    match db::steop_state_reset(
        &conn,
        &p.host,
        &p.project_dir,
        p.session_id(),
        &req.counter,
        value,
    ) {
        Ok(v) => Json(json!({ "counter": req.counter, "value": v })).into_response(),
        Err(e) => err500(e),
    }
}

async fn state_delete(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_state_delete(&conn, &p.host, &p.project_dir, p.session_id()) {
        Ok(deleted) => Json(json!({ "deleted": deleted })).into_response(),
        Err(e) => err500(e),
    }
}

async fn status_get(State(db): State<DbPool>, Json(req): Json<IdReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    match db::steop_status_get(&conn, &p.host, &p.project_dir, p.session_id()) {
        Ok(proj) => Json(proj).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_put(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let p = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let key = req.key.as_deref().unwrap_or("");
    let content = req.content.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = if p.is_project() {
        db::steop_storage_project_put(&conn, &p.host, &p.project_dir, key, content)
    } else {
        db::steop_storage_session_put(
            &conn,
            &p.host,
            &p.project_dir,
            p.session_id(),
            key,
            content,
        )
    };
    match res {
        Ok(m) => Json(m).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_get(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let p = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let key = req.key.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = if p.is_project() {
        db::steop_storage_project_get(&conn, &p.host, &p.project_dir, key)
    } else {
        db::steop_storage_session_get(&conn, &p.host, &p.project_dir, p.session_id(), key)
    };
    match res {
        Ok(Some(b)) => Json(b).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn storage_delete(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let p = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let key = req.key.as_deref().unwrap_or("");
    let conn = db.lock().await;
    let res = if p.is_project() {
        db::steop_storage_project_delete(&conn, &p.host, &p.project_dir, key)
    } else {
        db::steop_storage_session_delete(&conn, &p.host, &p.project_dir, p.session_id(), key)
    };
    match res {
        Ok(deleted) => Json(json!({ "deleted": deleted })).into_response(),
        Err(e) => err500(e),
    }
}

async fn storage_list(State(db): State<DbPool>, Json(req): Json<StorageReq>) -> Response {
    let p = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    let res = if p.is_project() {
        db::steop_storage_project_list(&conn, &p.host, &p.project_dir)
    } else {
        db::steop_storage_session_list(&conn, &p.host, &p.project_dir, p.session_id())
    };
    match res {
        Ok(items) => Json(json!({ "items": items })).into_response(),
        Err(e) => err500(e),
    }
}

async fn log_append(State(db): State<DbPool>, Json(req): Json<LogAppendReq>) -> Response {
    let p = match parse_full_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(e),
    };
    let conn = db.lock().await;
    let default_data = Value::Object(Default::default());
    let data = req.data.as_ref().unwrap_or(&default_data);
    match db::steop_log_append(
        &conn,
        &p.host,
        &p.project_dir,
        p.session_id(),
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

fn compose_id(p: &ParsedId) -> String {
    match &p.principal {
        Principal::Project => format!("{}:{}", p.host, p.project_dir),
        Principal::Session(sid) => format!("{}:{}:{}", p.host, p.project_dir, sid),
        Principal::User => format!("{}:{}:{}", p.host, p.project_dir, USER_LITERAL),
    }
}

async fn mailbox_send(State(db): State<DbPool>, Json(req): Json<MailboxSendReq>) -> Response {
    let caller = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(format!("id: {}", e)),
    };
    let from_id = match req.from.as_deref() {
        Some(explicit) => {
            if let Err(e) = parse_id(explicit) {
                return err400(format!("from: {}", e));
            }
            explicit.to_string()
        }
        None => compose_id(&caller),
    };
    if let Err(e) = parse_id(&req.to) {
        return err400(format!("to: {}", e));
    }

    let subject = req.subject.unwrap_or_default();
    let message_type = req
        .message_type
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "NOTE".to_string());
    let meta = req.meta.unwrap_or_else(|| Value::Object(Default::default()));
    let payload = req.payload.unwrap_or_else(|| Value::Object(Default::default()));

    let conn = db.lock().await;
    match db::steop_mailbox_send(
        &conn,
        &from_id,
        &req.to,
        &subject,
        &message_type,
        &meta,
        &payload,
    ) {
        Ok(row) => Json(row).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_list(State(db): State<DbPool>, Json(req): Json<MailboxListReq>) -> Response {
    let caller = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(format!("id: {}", e)),
    };
    let to_id = match req.to {
        Some(t) => {
            if let Err(e) = parse_id(&t) {
                return err400(format!("to: {}", e));
            }
            t
        }
        None => compose_id(&caller),
    };
    let statuses = req.status.unwrap_or_else(|| vec!["NEW".to_string()]);
    for s in &statuses {
        match s.as_str() {
            "NEW" | "READ" | "ARCHIVE" => {}
            other => return err400(format!("status: invalid value '{}'", other)),
        }
    }
    let limit = req.limit.unwrap_or(200).clamp(1, 1000);

    let conn = db.lock().await;
    match db::steop_mailbox_list(
        &conn,
        &to_id,
        &statuses,
        req.message_type.as_deref(),
        limit,
    ) {
        Ok(messages) => Json(json!({ "messages": messages })).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_get(State(db): State<DbPool>, Json(req): Json<MailboxRowReq>) -> Response {
    if let Err(e) = parse_id(&req.id) {
        return err400(format!("id: {}", e));
    }
    let conn = db.lock().await;
    match db::steop_mailbox_get(&conn, req.message_id) {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn mailbox_read(State(db): State<DbPool>, Json(req): Json<MailboxRowReq>) -> Response {
    if let Err(e) = parse_id(&req.id) {
        return err400(format!("id: {}", e));
    }
    let conn = db.lock().await;
    match db::steop_mailbox_read(&conn, req.message_id) {
        Ok(db::MailboxTransition::Ok) => {
            Json(json!({ "message_id": req.message_id, "status": "READ" })).into_response()
        }
        Ok(db::MailboxTransition::NotFound) => not_found(),
        Ok(db::MailboxTransition::Conflict(current)) => err409(format!(
            "invalid mailbox status transition: {} -> READ",
            current
        )),
        Err(e) => err500(e),
    }
}

async fn mailbox_update_meta(
    State(db): State<DbPool>,
    Json(req): Json<MailboxUpdateMetaReq>,
) -> Response {
    if let Err(e) = parse_id(&req.id) {
        return err400(format!("id: {}", e));
    }
    let mut conn = db.lock().await;
    match db::steop_mailbox_update_meta(&mut conn, req.message_id, req.meta_patch) {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => not_found(),
        Err(e) => err500(e),
    }
}

async fn mailbox_archive(State(db): State<DbPool>, Json(req): Json<MailboxRowReq>) -> Response {
    if let Err(e) = parse_id(&req.id) {
        return err400(format!("id: {}", e));
    }
    let conn = db.lock().await;
    match db::steop_mailbox_archive(&conn, req.message_id) {
        Ok(db::MailboxTransition::Ok) => Json(json!({
            "message_id": req.message_id,
            "status": "ARCHIVE"
        }))
        .into_response(),
        Ok(db::MailboxTransition::NotFound) => not_found(),
        Ok(db::MailboxTransition::Conflict(current)) => err409(format!(
            "invalid mailbox status transition: {} -> ARCHIVE",
            current
        )),
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
