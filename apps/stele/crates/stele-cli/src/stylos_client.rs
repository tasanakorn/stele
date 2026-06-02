//! Transient zenoh peer for the `stele mail` surface (PRD-027 §4.13).
//!
//! Opens a short-lived direct-connect peer (no multicast scouting) against the
//! local stele-server node, resolves this machine's node `<instance>` via the
//! `info` queryable, and issues a single `mailbox/<leaf>` GET, returning the raw
//! JSON reply.

use std::sync::Arc;
use std::time::Duration;

use stylos::{Endpoints, IdentitySection, SessionOverrides, StylosConfig, ZenohSection};
use zenoh::bytes::Encoding;
use zenoh::query::QueryTarget;

// Target is localhost / local LAN — round-trips are single-digit ms.
const QUERY_TIMEOUT: Duration = Duration::from_millis(500);

pub struct StylosClient {
    session: Arc<zenoh::Session>,
    realm: String,
}

impl StylosClient {
    /// Open a transient peer connecting directly to `zenoh_endpoint`.
    pub async fn open(zenoh_endpoint: &str, realm: &str) -> Result<Self, String> {
        let cfg = StylosConfig {
            stylos: IdentitySection {
                realm: realm.to_string(),
                role: "stele-cli".to_string(),
                instance: "stele-cli".to_string(),
            },
            zenoh: ZenohSection {
                // The CLI is a pure client of the local router node, not a mesh
                // peer. Client mode connects to the node, queries, and closes
                // promptly — peer mode lingers ~10s on close (its session-close
                // timeout) which dominated every `stele mail` invocation.
                mode: "client".to_string(),
                connect: Endpoints {
                    endpoints: vec![zenoh_endpoint.to_string()],
                },
                listen: Endpoints::default(),
                scouting: None,
            },
        };
        let overrides = SessionOverrides {
            connect: Some(vec![zenoh_endpoint.to_string()]),
        };
        let session = stylos::open_session(&cfg, &overrides)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self {
            session: Arc::new(session),
            realm: realm.to_string(),
        })
    }

    /// Resolve THIS machine's node `<instance>` by GET-ing `…/*/info` and
    /// picking the reply whose `mailbox_host` equals this host's `gethostname`.
    /// If exactly one info reply is seen, use it regardless.
    pub async fn resolve_local_instance(&self) -> Result<String, String> {
        // STELE_HOST override (steop precedence), else gethostname — must match
        // the node's `mailbox_host` claim for local-node resolution to succeed.
        let local_host = std::env::var("STELE_HOST")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned());
        let key = format!("stylos/{}/stele/*/info", self.realm);
        let replies = self
            .session
            .get(&key)
            .timeout(QUERY_TIMEOUT)
            .target(QueryTarget::All)
            .await
            .map_err(|e| e.to_string())?;

        let mut candidates: Vec<(String, String)> = Vec::new();
        while let Ok(reply) = replies.recv_async().await {
            let sample = match reply.into_result() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let s = match sample.payload().try_to_string() {
                Ok(s) => s.into_owned(),
                Err(_) => continue,
            };
            let v: serde_json::Value = match serde_json::from_str(&s) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let inst = v
                .get("instance")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let host = v
                .get("mailbox_host")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if inst.is_empty() {
                continue;
            }
            if host.eq_ignore_ascii_case(&local_host) {
                return Ok(inst);
            }
            candidates.push((inst, host));
        }

        match candidates.len() {
            0 => Err("no local stele node found (no info reply)".to_string()),
            1 => Ok(candidates.remove(0).0),
            _ => Err(format!(
                "could not resolve local node: {} nodes responded, none claims host '{}'",
                candidates.len(),
                local_host
            )),
        }
    }

    /// Issue one `mailbox/<leaf>` GET with `request` as the JSON payload and
    /// return the parsed JSON reply.
    pub async fn query_leaf(
        &self,
        instance: &str,
        leaf: &str,
        request: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let key = format!("stylos/{}/stele/{}/mailbox/{}", self.realm, instance, leaf);
        let body = serde_json::to_vec(request).map_err(|e| e.to_string())?;
        let replies = self
            .session
            .get(&key)
            .payload(body)
            .encoding(Encoding::APPLICATION_JSON)
            .timeout(QUERY_TIMEOUT)
            .target(QueryTarget::All)
            .await
            .map_err(|e| e.to_string())?;

        while let Ok(reply) = replies.recv_async().await {
            let sample = match reply.into_result() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let s = sample.payload().try_to_string().map_err(|e| e.to_string())?;
            return serde_json::from_str(&s).map_err(|e| e.to_string());
        }
        Err("no reply from local node".to_string())
    }

    pub async fn close(self) {
        self.session.close().await.ok();
    }
}
