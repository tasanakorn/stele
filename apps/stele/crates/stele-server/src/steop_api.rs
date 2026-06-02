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

// ── Request types ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MailboxSendReq {
    id: String,
    to: String,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    attention: Option<String>,
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
    #[serde(default)]
    attention: Option<String>,
    #[serde(default, deserialize_with = "string_or_string_vec_opt")]
    status: Option<Vec<String>>,
    #[serde(default)]
    #[allow(dead_code)]
    message_type: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
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
    let to = match parse_id(&req.to) {
        Ok(p) => p,
        Err(e) => return err400(format!("to: {}", e)),
    };

    let subject = req.subject.unwrap_or_default();
    let message_type = req
        .message_type
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "NOTE".to_string());
    let meta = req.meta.unwrap_or_else(|| Value::Object(Default::default()));
    let payload = req.payload.unwrap_or_else(|| Value::Object(Default::default()));
    let mail_uid = ulid::Ulid::new().to_string();
    let attention = req.attention.filter(|s| !s.trim().is_empty());

    let conn = db.lock().await;
    match db::mailbox_inbox_upsert(
        &conn,
        &mail_uid,
        &to.project_dir,
        attention.as_deref(),
        &from_id,
        &subject,
        &message_type,
        &meta,
        &payload,
        "NEW",
    ) {
        Ok((message_id, _)) => match db::mailbox_inbox_get(&conn, message_id) {
            Ok(Some(row)) => Json(row).into_response(),
            Ok(None) => err500("row just inserted not found"),
            Err(e) => err500(e),
        },
        Err(e) => err500(e),
    }
}

async fn mailbox_list(State(db): State<DbPool>, Json(req): Json<MailboxListReq>) -> Response {
    let caller = match parse_id(&req.id) {
        Ok(p) => p,
        Err(e) => return err400(format!("id: {}", e)),
    };
    let to = match req.to {
        Some(t) => match parse_id(&t) {
            Ok(p) => p,
            Err(e) => return err400(format!("to: {}", e)),
        },
        None => caller,
    };
    let statuses = req.status.unwrap_or_else(|| vec!["NEW".to_string()]);
    for s in &statuses {
        match s.as_str() {
            "NEW" | "READ" | "ARCHIVE" => {}
            other => return err400(format!("status: invalid value '{}'", other)),
        }
    }
    // REST callers select by a single optional attention. When absent, only
    // household/broadcast rows are visible (backward-compatible).
    let aliases: Vec<String> = req
        .attention
        .filter(|s| !s.trim().is_empty())
        .map(|a| vec![a])
        .unwrap_or_default();

    let conn = db.lock().await;
    match db::mailbox_inbox_list(&conn, &to.project_dir, &aliases, &statuses) {
        Ok(messages) => Json(json!({ "messages": messages })).into_response(),
        Err(e) => err500(e),
    }
}

async fn mailbox_get(State(db): State<DbPool>, Json(req): Json<MailboxRowReq>) -> Response {
    if let Err(e) = parse_id(&req.id) {
        return err400(format!("id: {}", e));
    }
    let conn = db.lock().await;
    match db::mailbox_inbox_get(&conn, req.message_id) {
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
    match db::mailbox_inbox_read(&conn, req.message_id) {
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
    match db::mailbox_inbox_update_meta(&mut conn, req.message_id, req.meta_patch) {
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
    match db::mailbox_inbox_archive(&conn, req.message_id) {
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
