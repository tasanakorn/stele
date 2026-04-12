use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use rand::RngCore;
use std::sync::Arc;
use tokio::sync::RwLock;

pub const HEADER_NAME: &str = "x-stele-key";

/// Shared, live-rotatable auth key state.
#[derive(Default)]
pub struct AuthState {
    pub key: RwLock<Option<String>>,
}

impl AuthState {
    pub fn new(key: Option<String>) -> Arc<Self> {
        Arc::new(Self { key: RwLock::new(key) })
    }

    /// Blocking read helper for the tray thread (no tokio runtime there).
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub fn blocking_get(&self) -> Option<String> {
        self.key.blocking_read().clone()
    }

    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub fn blocking_set(&self, v: Option<String>) {
        *self.key.blocking_write() = v;
    }
}

/// Generate a new key: 32 random bytes, URL-safe base64 no-pad (43 chars).
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub fn generate_key() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

/// Constant-time byte compare.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// axum middleware: enforce X-Stele-Key when configured.
pub async fn auth_layer(
    State(state): State<Arc<AuthState>>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let expected = { state.key.read().await.clone() };
    let Some(expected) = expected else {
        return next.run(req).await; // auth disabled
    };

    let presented = req
        .headers()
        .get(HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if ct_eq(presented.as_bytes(), expected.as_bytes()) {
        next.run(req).await
    } else {
        unauthorized()
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": "unauthorized" })),
    )
        .into_response()
}
