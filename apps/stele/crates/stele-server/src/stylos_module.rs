#![cfg(feature = "stylos")]
//! Stylos session lifecycle inside stele-server.
//!
//! Owns an `Arc<zenoh::Session>`, a heartbeat publisher task, and an info
//! queryable task. Tied into the axum shutdown path via a CancellationToken.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use stylos_config::{Endpoints, IdentitySection, StylosConfig, ZenohSection};
use stylos_session::SessionOverrides;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use zenoh::bytes::Encoding;
use zenoh::qos::CongestionControl;

use crate::settings::StyloSettings;

/// Opaque source for the current stylos health; cheap to clone (internally Arc-ed).
pub struct StylosStatusSource {
    pub session: Arc<zenoh::Session>,
    pub mode: String,
    pub realm: String,
    pub instance: String,
    pub zid_short: String,
    pub started_at: String,
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
    session: Arc<zenoh::Session>,
}

impl StylosHandle {
    pub async fn shutdown(self) {
        self.heartbeat_task.abort();
        self.queryable_task.abort();
        let _ = self.heartbeat_task.await;
        let _ = self.queryable_task.await;
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

    let session = Arc::new(stylos_session::open_session(&cfg, &overrides).await?);

    let zid_full = session.info().zid().await.to_string();
    if instance == "__zid_pending__" {
        let short: String = zid_full.chars().take(8).collect();
        instance = format!("stele-{short}");
    }
    let zid_short: String = zid_full.chars().take(8).collect();
    let started_at = chrono::Utc::now().to_rfc3339();

    let status = Arc::new(StylosStatusSource {
        session: session.clone(),
        mode: settings.mode.clone(),
        realm: settings.realm.clone(),
        instance: instance.clone(),
        zid_short: zid_short.clone(),
        started_at: started_at.clone(),
    });

    // Heartbeat task
    let hb_key = format!(
        "stylos/{}/stele/{}/heartbeat",
        settings.realm, instance
    );
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

    // Queryable task
    let q_key = format!("stylos/{}/stele/{}/info", settings.realm, instance);
    let q_session = session.clone();
    let q_ct = ct.clone();
    let q_mode = settings.mode.clone();
    let q_realm = settings.realm.clone();
    let q_instance = instance.clone();
    let q_zid = zid_full.clone();
    let q_started = started_at.clone();
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
                            "version": env!("CARGO_PKG_VERSION"),
                            "stylos_version": "0.1.0",
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

    tracing::info!(
        zid = %zid_full,
        mode = %settings.mode,
        realm = %settings.realm,
        instance = %instance,
        "stylos session ready"
    );

    Ok(StylosHandle {
        status,
        heartbeat_task,
        queryable_task,
        session,
    })
}
