#![cfg(feature = "stylos")]
//! Stylos session lifecycle inside stele-server.
//!
//! Owns an `Arc<zenoh::Session>`, a heartbeat publisher task, an info
//! queryable task, the postal mailbox queryables (PRD-027), an origin
//! delivery/retry worker, and a heartbeat-listener reachability map. Tied
//! into the axum shutdown path via a CancellationToken.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use stylos::{Endpoints, IdentitySection, SessionOverrides, StylosConfig, ZenohSection};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use zenoh::bytes::Encoding;
use zenoh::qos::CongestionControl;

use crate::db::{self, DbPool};
use crate::settings::StyloSettings;

// ── Delivery / retry worker constants (PRD-027 §4.8) ──────────────────────────

const WORKER_TICK: Duration = Duration::from_secs(5);
const BACKOFF_BASE: Duration = Duration::from_secs(5);
const BACKOFF_MULT: u32 = 2;
const BACKOFF_CAP: Duration = Duration::from_secs(300);
const MAX_ATTEMPTS: i64 = 50;
const OUTBOX_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
// Localhost / local-LAN target; a miss just retries next tick (idempotent).
const DELIVER_TIMEOUT: Duration = Duration::from_millis(500);
const HEARTBEAT_FRESH: Duration = Duration::from_secs(15);

/// `backoff(attempts) = min(CAP, BASE * MULT^(attempts-1))`.
fn backoff(attempts: i64) -> Duration {
    if attempts <= 1 {
        return BACKOFF_BASE.min(BACKOFF_CAP);
    }
    let exp = (attempts - 1) as u32;
    let factor = BACKOFF_MULT.checked_pow(exp);
    match factor {
        Some(f) => BACKOFF_BASE
            .checked_mul(f)
            .unwrap_or(BACKOFF_CAP)
            .min(BACKOFF_CAP),
        None => BACKOFF_CAP,
    }
}

/// Shared instance→last_seen reachability map.
type ReachMap = Arc<AsyncMutex<HashMap<String, Instant>>>;

/// Opaque source for the current stylos health; cheap to clone (internally Arc-ed).
pub struct StylosStatusSource {
    pub session: Arc<zenoh::Session>,
    pub mode: String,
    pub realm: String,
    pub instance: String,
    pub zid_short: String,
    pub started_at: String,
    /// Raw, un-normalized hostname this node claims as its mailbox host.
    pub mailbox_host: String,
}

impl StylosStatusSource {
    pub async fn to_health(&self) -> StylosHealth {
        let info = self.session.info();
        let zid = info.zid().await.to_string();
        let peers = info.peers_zid().await.count();
        let routers = info.routers_zid().await.count();
        StylosHealth {
            enabled: true,
            mode: self.mode.clone(),
            zid,
            realm: self.realm.clone(),
            instance: self.instance.clone(),
            listen_endpoints: Vec::new(),
            peers,
            routers,
        }
    }
}

#[derive(Serialize)]
pub struct StylosHealth {
    pub enabled: bool,
    pub mode: String,
    pub zid: String,
    pub realm: String,
    pub instance: String,
    pub listen_endpoints: Vec<String>,
    pub peers: usize,
    pub routers: usize,
}

pub struct StylosHandle {
    pub status: Arc<StylosStatusSource>,
    heartbeat_task: JoinHandle<()>,
    queryable_task: JoinHandle<()>,
    mailbox_task: JoinHandle<()>,
    delivery_task: JoinHandle<()>,
    heartbeat_listener_task: JoinHandle<()>,
    session: Arc<zenoh::Session>,
}

impl StylosHandle {
    pub async fn shutdown(self) {
        self.heartbeat_task.abort();
        self.queryable_task.abort();
        self.mailbox_task.abort();
        self.delivery_task.abort();
        self.heartbeat_listener_task.abort();
        let _ = self.heartbeat_task.await;
        let _ = self.queryable_task.await;
        let _ = self.mailbox_task.await;
        let _ = self.delivery_task.await;
        let _ = self.heartbeat_listener_task.await;
        if let Err(e) = self.session.close().await {
            tracing::warn!("stylos session close error: {e}");
        }
    }
}

/// Derive an `<instance>` segment.
///
/// Priority: explicit override (trimmed, verbatim) → normalized hostname →
/// `None` (caller falls back to `stele-<short-zid>` after session open).
pub fn derive_instance(override_id: Option<&str>) -> Option<String> {
    if let Some(o) = override_id {
        let t = o.trim();
        if !t.is_empty() {
            if is_valid_instance(t) {
                return Some(t.to_string());
            } else {
                tracing::warn!(
                    override_id = %t,
                    "stylos: STELE_STYLOS_INSTANCE override does not match grammar [a-z0-9][a-z0-9-]* — falling back to hostname"
                );
            }
        }
    }
    let hn = hostname::get().ok()?.to_string_lossy().into_owned();
    let lower = hn.to_lowercase();
    let mapped: String = lower
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = mapped.trim_matches('-').to_string();
    let capped: String = trimmed.chars().take(32).collect();
    if capped.is_empty() {
        None
    } else {
        Some(capped)
    }
}

fn is_valid_instance(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn assemble_stylos_config(settings: &StyloSettings, instance: &str) -> StylosConfig {
    StylosConfig {
        stylos: IdentitySection {
            realm: settings.realm.clone(),
            role: "stele".to_string(),
            instance: instance.to_string(),
        },
        zenoh: ZenohSection {
            mode: settings.mode.clone(),
            connect: Endpoints {
                endpoints: settings.connect.clone(),
            },
            listen: Endpoints::default(),
            scouting: None,
        },
    }
}

pub async fn start(
    settings: &StyloSettings,
    ct: CancellationToken,
    pool: DbPool,
) -> Result<StylosHandle, Box<dyn std::error::Error + Send + Sync>> {
    let mut instance = derive_instance(settings.instance.as_deref())
        .unwrap_or_else(|| "__zid_pending__".to_string());

    let cfg = assemble_stylos_config(settings, &instance);

    let overrides = SessionOverrides {
        connect: if settings.connect.is_empty() {
            None
        } else {
            Some(settings.connect.clone())
        },
    };

    let session = Arc::new(stylos::open_session(&cfg, &overrides).await?);

    let zid_full = session.info().zid().await.to_string();
    if instance == "__zid_pending__" {
        let short: String = zid_full.chars().take(8).collect();
        instance = format!("stele-{short}");
    }
    let zid_short: String = zid_full.chars().take(8).collect();
    let started_at = chrono::Utc::now().to_rfc3339();

    // The node's mailbox-host claim (PRD-027 §4.11): `STELE_HOST` override, else
    // the raw un-normalized hostname. Mirrors steop's host precedence so the
    // CLI (which uses the same precedence) resolves this node deterministically.
    let mailbox_host = std::env::var("STELE_HOST")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
        .or_else(|| hostname::get().ok().map(|h| h.to_string_lossy().into_owned()))
        .unwrap_or_else(|| instance.clone());

    let status = Arc::new(StylosStatusSource {
        session: session.clone(),
        mode: settings.mode.clone(),
        realm: settings.realm.clone(),
        instance: instance.clone(),
        zid_short: zid_short.clone(),
        started_at: started_at.clone(),
        mailbox_host: mailbox_host.clone(),
    });

    // Heartbeat task
    let hb_key = format!("stylos/{}/stele/{}/heartbeat", settings.realm, instance);
    let hb_session = session.clone();
    let hb_ct = ct.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = hb_ct.cancelled() => break,
                _ = interval.tick() => {
                    let res = hb_session
                        .put(&hb_key, b"alive".to_vec())
                        .encoding(Encoding::APPLICATION_OCTET_STREAM)
                        .congestion_control(CongestionControl::Drop)
                        .await;
                    if let Err(e) = res {
                        tracing::warn!(key = %hb_key, "stylos heartbeat put failed: {e}");
                    }
                }
            }
        }
    });

    // Info queryable task
    let q_key = format!("stylos/{}/stele/{}/info", settings.realm, instance);
    let q_session = session.clone();
    let q_ct = ct.clone();
    let q_mode = settings.mode.clone();
    let q_realm = settings.realm.clone();
    let q_instance = instance.clone();
    let q_zid = zid_full.clone();
    let q_started = started_at.clone();
    let q_mailbox_host = mailbox_host.clone();
    let queryable_task = tokio::spawn(async move {
        let queryable = match q_session.declare_queryable(&q_key).await {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(key = %q_key, "stylos declare_queryable failed: {e}");
                return;
            }
        };
        loop {
            tokio::select! {
                _ = q_ct.cancelled() => break,
                res = queryable.recv_async() => match res {
                    Ok(query) => {
                        let body = serde_json::json!({
                            "zid": q_zid,
                            "mode": q_mode,
                            "realm": q_realm,
                            "instance": q_instance,
                            "mailbox_host": q_mailbox_host,
                            "version": env!("CARGO_PKG_VERSION"),
                            "stylos_version": stylos::VERSION,
                            "listen_endpoints": Vec::<String>::new(),
                            "started_at": q_started,
                        });
                        let payload = serde_json::to_vec(&body).unwrap_or_default();
                        if let Err(e) = query
                            .reply(q_key.clone(), payload)
                            .encoding(Encoding::APPLICATION_JSON)
                            .await
                        {
                            tracing::warn!("stylos queryable reply err: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("stylos queryable recv err: {e}");
                        break;
                    }
                }
            }
        }
    });

    // Mailbox queryables (PRD-027 §4.3/§4.5)
    let mailbox_task = spawn_mailbox_queryables(
        session.clone(),
        ct.clone(),
        pool.clone(),
        settings.realm.clone(),
        instance.clone(),
        mailbox_host.clone(),
    );

    // Reachability map shared between heartbeat listener and delivery worker.
    let reach: ReachMap = Arc::new(AsyncMutex::new(HashMap::new()));

    let heartbeat_listener_task = spawn_heartbeat_listener(
        session.clone(),
        ct.clone(),
        reach.clone(),
        settings.realm.clone(),
    );

    let delivery_task = spawn_delivery_worker(
        session.clone(),
        ct.clone(),
        pool.clone(),
        reach.clone(),
        settings.realm.clone(),
    );

    tracing::info!(
        zid = %zid_full,
        mode = %settings.mode,
        realm = %settings.realm,
        instance = %instance,
        mailbox_host = %mailbox_host,
        "stylos session ready"
    );

    Ok(StylosHandle {
        status,
        heartbeat_task,
        queryable_task,
        mailbox_task,
        delivery_task,
        heartbeat_listener_task,
        session,
    })
}

// ── Mailbox queryable structs (PRD-027 §4.4/§4.5) ─────────────────────────────

#[derive(serde::Deserialize)]
struct SendReq {
    to_host: String,
    to_project: String,
    #[serde(default)]
    attention: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    message_type: Option<String>,
    #[serde(default)]
    meta: Option<serde_json::Value>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
    from: String,
}

#[derive(serde::Deserialize)]
struct DeliverReq {
    mail_uid: String,
    to_project: String,
    #[serde(default)]
    attention: Option<String>,
    from: String,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    message_type: Option<String>,
    #[serde(default)]
    meta: Option<serde_json::Value>,
    #[serde(default)]
    payload: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct ListReq {
    project_dir: String,
    #[serde(default)]
    aliases: Option<Vec<String>>,
    #[serde(default)]
    status: Option<Vec<String>>,
    #[serde(default)]
    register_only: bool,
}

#[derive(serde::Deserialize)]
struct MsgIdReq {
    message_id: i64,
}

#[derive(serde::Deserialize)]
struct OutboxReq {
    #[serde(default)]
    status: Option<Vec<String>>,
}

fn ok_envelope(mut fields: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut m) = fields {
        m.insert("ok".to_string(), serde_json::Value::Bool(true));
    }
    fields
}

fn err_envelope(code: &str, reason: impl std::fmt::Display) -> serde_json::Value {
    serde_json::json!({ "ok": false, "error": code, "reason": reason.to_string() })
}

fn default_value() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

#[allow(clippy::too_many_arguments)]
fn spawn_mailbox_queryables(
    session: Arc<zenoh::Session>,
    ct: CancellationToken,
    pool: DbPool,
    realm: String,
    instance: String,
    mailbox_host: String,
) -> JoinHandle<()> {
    let key = format!("stylos/{}/stele/{}/mailbox/*", realm, instance);
    tokio::spawn(async move {
        let queryable = match session.declare_queryable(&key).await {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(key = %key, "stylos mailbox declare_queryable failed: {e}");
                return;
            }
        };
        loop {
            tokio::select! {
                _ = ct.cancelled() => break,
                res = queryable.recv_async() => match res {
                    Ok(query) => {
                        let key_str = query.key_expr().as_str().to_string();
                        let leaf = key_str.rsplit('/').next().unwrap_or("").to_string();
                        let body = query
                            .payload()
                            .and_then(|p| p.try_to_string().ok().map(|c| c.into_owned()))
                            .unwrap_or_default();
                        let reply = handle_mailbox_leaf(&leaf, &body, &pool, &mailbox_host).await;
                        let payload = serde_json::to_vec(&reply).unwrap_or_default();
                        if let Err(e) = query
                            .reply(key_str, payload)
                            .encoding(Encoding::APPLICATION_JSON)
                            .await
                        {
                            tracing::warn!("stylos mailbox reply err: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("stylos mailbox recv err: {e}");
                        break;
                    }
                }
            }
        }
    })
}

async fn handle_mailbox_leaf(
    leaf: &str,
    body: &str,
    pool: &DbPool,
    mailbox_host: &str,
) -> serde_json::Value {
    match leaf {
        "send" => mailbox_send_leaf(body, pool, mailbox_host).await,
        "deliver" => mailbox_deliver_leaf(body, pool).await,
        "list" => mailbox_list_leaf(body, pool).await,
        "read" => mailbox_read_leaf(body, pool).await,
        "archive" => mailbox_archive_leaf(body, pool).await,
        "get" => mailbox_get_leaf(body, pool).await,
        "outbox" => mailbox_outbox_leaf(body, pool).await,
        other => err_envelope("bad_request", format!("unknown mailbox leaf '{}'", other)),
    }
}

async fn mailbox_send_leaf(
    body: &str,
    pool: &DbPool,
    mailbox_host: &str,
) -> serde_json::Value {
    let req: SendReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let mail_uid = ulid::Ulid::new().to_string();
    let subject = req.subject.unwrap_or_default();
    let message_type = req
        .message_type
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "NOTE".to_string());
    let meta = req.meta.unwrap_or_else(default_value);
    let payload = req.payload.unwrap_or_else(default_value);
    let attention = req.attention.filter(|s| !s.trim().is_empty());

    let conn = pool.lock().await;
    if req.to_host.eq_ignore_ascii_case(mailbox_host) {
        // Self-host: store directly into the inbox.
        match db::mailbox_inbox_upsert(
            &conn,
            &mail_uid,
            &req.to_project,
            attention.as_deref(),
            &req.from,
            &subject,
            &message_type,
            &meta,
            &payload,
            "NEW",
        ) {
            Ok((message_id, _)) => ok_envelope(serde_json::json!({
                "mail_uid": mail_uid,
                "message_id": message_id,
                "status": "delivered",
            })),
            Err(e) => err_envelope("internal", e),
        }
    } else {
        // Remote: spool for the delivery worker.
        match db::mailbox_outbox_enqueue(
            &conn,
            &mail_uid,
            &req.to_host,
            &req.to_project,
            attention.as_deref(),
            &req.from,
            &subject,
            &message_type,
            &meta,
            &payload,
        ) {
            Ok(()) => ok_envelope(serde_json::json!({
                "mail_uid": mail_uid,
                "status": "queued",
            })),
            Err(e) => err_envelope("internal", e),
        }
    }
}

async fn mailbox_deliver_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: DeliverReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let subject = req.subject.unwrap_or_default();
    let message_type = req
        .message_type
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "NOTE".to_string());
    let meta = req.meta.unwrap_or_else(default_value);
    let payload = req.payload.unwrap_or_else(default_value);
    let attention = req.attention.filter(|s| !s.trim().is_empty());

    let conn = pool.lock().await;
    match db::mailbox_inbox_upsert(
        &conn,
        &req.mail_uid,
        &req.to_project,
        attention.as_deref(),
        &req.from,
        &subject,
        &message_type,
        &meta,
        &payload,
        "NEW",
    ) {
        Ok((message_id, status)) => ok_envelope(serde_json::json!({
            "message_id": message_id,
            "status": status,
        })),
        Err(e) => err_envelope("internal", e),
    }
}

async fn mailbox_list_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: ListReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let aliases = req.aliases.unwrap_or_default();
    let conn = pool.lock().await;

    // Upsert any supplied aliases (the `register` mechanism, §4.6).
    let registered = if aliases.is_empty() {
        Vec::new()
    } else {
        match db::mailbox_alias_upsert(&conn, &req.project_dir, &aliases) {
            Ok(r) => r,
            Err(e) => return err_envelope("internal", e),
        }
    };

    if req.register_only {
        return ok_envelope(serde_json::json!({ "registered": registered }));
    }

    let statuses = req
        .status
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["NEW".to_string(), "READ".to_string()]);
    for s in &statuses {
        match s.as_str() {
            "NEW" | "READ" | "ARCHIVE" => {}
            other => return err_envelope("bad_request", format!("invalid status '{}'", other)),
        }
    }

    // The caller asserts its identity via the supplied aliases (the `--alias`
    // flags). Filter by exactly those; fall back to the project's persisted
    // registry only when the caller supplies none. This keeps `attention` a
    // per-caller assertion rather than leaking every project-registered alias
    // to every lister of the project.
    let caller_aliases = if aliases.is_empty() {
        match db::mailbox_alias_list(&conn, &req.project_dir) {
            Ok(a) => a,
            Err(e) => return err_envelope("internal", e),
        }
    } else {
        aliases
    };

    match db::mailbox_inbox_list(&conn, &req.project_dir, &caller_aliases, &statuses) {
        Ok(messages) => ok_envelope(serde_json::json!({ "messages": messages })),
        Err(e) => err_envelope("internal", e),
    }
}

async fn mailbox_read_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: MsgIdReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let conn = pool.lock().await;
    match db::mailbox_inbox_read(&conn, req.message_id) {
        Ok(db::MailboxTransition::Ok) => ok_envelope(serde_json::json!({
            "message_id": req.message_id,
            "status": "READ",
        })),
        Ok(db::MailboxTransition::NotFound) => err_envelope("not_found", "message not found"),
        Ok(db::MailboxTransition::Conflict(current)) => err_envelope(
            "conflict",
            format!("invalid transition: {} -> READ", current),
        ),
        Err(e) => err_envelope("internal", e),
    }
}

async fn mailbox_archive_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: MsgIdReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let conn = pool.lock().await;
    match db::mailbox_inbox_archive(&conn, req.message_id) {
        Ok(db::MailboxTransition::Ok) => ok_envelope(serde_json::json!({
            "message_id": req.message_id,
            "status": "ARCHIVE",
        })),
        Ok(db::MailboxTransition::NotFound) => err_envelope("not_found", "message not found"),
        Ok(db::MailboxTransition::Conflict(current)) => err_envelope(
            "conflict",
            format!("invalid transition: {} -> ARCHIVE", current),
        ),
        Err(e) => err_envelope("internal", e),
    }
}

async fn mailbox_get_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: MsgIdReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let conn = pool.lock().await;
    match db::mailbox_inbox_get(&conn, req.message_id) {
        Ok(Some(row)) => ok_envelope(serde_json::json!({ "message": row })),
        Ok(None) => err_envelope("not_found", "message not found"),
        Err(e) => err_envelope("internal", e),
    }
}

async fn mailbox_outbox_leaf(body: &str, pool: &DbPool) -> serde_json::Value {
    let req: OutboxReq = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return err_envelope("bad_request", e),
    };
    let statuses = req
        .status
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["QUEUED".to_string(), "DEAD".to_string()]);
    let conn = pool.lock().await;
    match db::mailbox_outbox_list(&conn, &statuses) {
        Ok(rows) => ok_envelope(serde_json::json!({ "rows": rows })),
        Err(e) => err_envelope("internal", e),
    }
}

// ── Heartbeat listener (PRD-027 §4.9) ─────────────────────────────────────────

fn spawn_heartbeat_listener(
    session: Arc<zenoh::Session>,
    ct: CancellationToken,
    reach: ReachMap,
    realm: String,
) -> JoinHandle<()> {
    let key = format!("stylos/{}/stele/*/heartbeat", realm);
    tokio::spawn(async move {
        let sub = match session.declare_subscriber(&key).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(key = %key, "stylos heartbeat subscriber failed: {e}");
                return;
            }
        };
        loop {
            tokio::select! {
                _ = ct.cancelled() => break,
                res = sub.recv_async() => match res {
                    Ok(sample) => {
                        // key = stylos/{realm}/stele/{instance}/heartbeat
                        let k = sample.key_expr().as_str();
                        let parts: Vec<&str> = k.split('/').collect();
                        // [stylos, realm, stele, instance, heartbeat]
                        if parts.len() >= 5 {
                            let inst = parts[3].to_string();
                            let mut map = reach.lock().await;
                            map.insert(inst, Instant::now());
                        }
                    }
                    Err(e) => {
                        tracing::warn!("stylos heartbeat sub recv err: {e}");
                        break;
                    }
                }
            }
        }
    })
}

// ── Delivery / retry worker (PRD-027 §4.8) ────────────────────────────────────

fn spawn_delivery_worker(
    session: Arc<zenoh::Session>,
    ct: CancellationToken,
    pool: DbPool,
    reach: ReachMap,
    realm: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(WORKER_TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = ct.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(e) = worker_sweep(&session, &pool, &reach, &realm).await {
                        tracing::warn!("stylos delivery sweep err: {e}");
                    }
                }
            }
        }
    })
}

async fn worker_sweep(
    session: &Arc<zenoh::Session>,
    pool: &DbPool,
    reach: &ReachMap,
    realm: &str,
) -> Result<(), String> {
    let now = chrono::Utc::now();
    let now_s = now.to_rfc3339();
    let due = {
        let conn = pool.lock().await;
        db::mailbox_outbox_due(&conn, &now_s).map_err(|e| e.to_string())?
    };
    if due.is_empty() {
        return Ok(());
    }

    // Snapshot fresh instances from the reachability map.
    let fresh: Vec<String> = {
        let map = reach.lock().await;
        let now_i = Instant::now();
        map.iter()
            .filter(|(_, &t)| now_i.duration_since(t) <= HEARTBEAT_FRESH)
            .map(|(k, _)| k.clone())
            .collect()
    };

    for row in due {
        // Resolve to_host → dest-instance among fresh instances via info GET.
        let dest = match resolve_dest_instance(session, realm, &row.to_host, &fresh).await {
            Some(d) => d,
            None => continue, // no reachable claiming node — skip, no attempt burned
        };

        let deliver_key = format!("stylos/{}/stele/{}/mailbox/deliver", realm, dest);
        let req = serde_json::json!({
            "mail_uid": row.mail_uid,
            "to_project": row.to_project,
            "attention": row.attention,
            "from": row.from_addr,
            "subject": row.subject,
            "message_type": row.message_type,
            "meta": row.meta,
            "payload": row.payload,
        });
        let req_bytes = serde_json::to_vec(&req).unwrap_or_default();

        let outcome = deliver_once(session, &deliver_key, req_bytes).await;
        let conn = pool.lock().await;
        match outcome {
            Ok(remote_message_id) => {
                let delivered_at = chrono::Utc::now().to_rfc3339();
                let _ = db::mailbox_outbox_mark(
                    &conn,
                    &row.mail_uid,
                    "DELIVERED",
                    row.attempts,
                    &row.next_attempt_at,
                    None,
                    remote_message_id,
                    Some(&delivered_at),
                );
            }
            Err(reason) => {
                let attempts = row.attempts + 1;
                let created = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map(|d| d.with_timezone(&chrono::Utc));
                let aged_out = match created {
                    Ok(c) => (now - c).to_std().map(|d| d > OUTBOX_TTL).unwrap_or(false),
                    Err(_) => false,
                };
                if attempts >= MAX_ATTEMPTS || aged_out {
                    let _ = db::mailbox_outbox_mark(
                        &conn,
                        &row.mail_uid,
                        "DEAD",
                        attempts,
                        &row.next_attempt_at,
                        Some(&reason),
                        None,
                        None,
                    );
                } else {
                    let next = (now + chrono::Duration::from_std(backoff(attempts))
                        .unwrap_or_else(|_| chrono::Duration::seconds(300)))
                    .to_rfc3339();
                    let _ = db::mailbox_outbox_mark(
                        &conn,
                        &row.mail_uid,
                        "QUEUED",
                        attempts,
                        &next,
                        Some(&reason),
                        None,
                        None,
                    );
                }
            }
        }
    }
    Ok(())
}

/// GET `…/*/info` and pick the fresh instance whose `mailbox_host == to_host`.
async fn resolve_dest_instance(
    session: &Arc<zenoh::Session>,
    realm: &str,
    to_host: &str,
    fresh: &[String],
) -> Option<String> {
    let key = format!("stylos/{}/stele/*/info", realm);
    let replies = session
        .get(&key)
        .timeout(DELIVER_TIMEOUT)
        .await
        .ok()?;
    while let Ok(reply) = replies.recv_async().await {
        let sample = match reply.into_result() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let s = match sample.payload().try_to_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let v: serde_json::Value = match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let inst = v.get("instance").and_then(|x| x.as_str()).unwrap_or("");
        let host = v.get("mailbox_host").and_then(|x| x.as_str()).unwrap_or("");
        if host.eq_ignore_ascii_case(to_host) && fresh.iter().any(|f| f == inst) {
            return Some(inst.to_string());
        }
    }
    None
}

/// Issue one `deliver` GET. Returns `Ok(remote_message_id)` on a `{ok:true}`
/// reply, else `Err(reason)`.
async fn deliver_once(
    session: &Arc<zenoh::Session>,
    key: &str,
    req_bytes: Vec<u8>,
) -> Result<Option<i64>, String> {
    let replies = session
        .get(key)
        .payload(req_bytes)
        .encoding(Encoding::APPLICATION_JSON)
        .timeout(DELIVER_TIMEOUT)
        .await
        .map_err(|e| e.to_string())?;
    match replies.recv_async().await {
        Ok(reply) => {
            let sample = reply.into_result().map_err(|e| e.to_string())?;
            let s = sample
                .payload()
                .try_to_string()
                .map_err(|e| e.to_string())?;
            let v: serde_json::Value =
                serde_json::from_str(&s).map_err(|e| e.to_string())?;
            if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
                let rmid = v.get("message_id").and_then(|x| x.as_i64());
                Ok(rmid)
            } else {
                let reason = v
                    .get("reason")
                    .and_then(|x| x.as_str())
                    .unwrap_or("delivery rejected")
                    .to_string();
                Err(reason)
            }
        }
        Err(_) => Err("timeout".to_string()),
    }
}
