mod api;
mod config;
mod db;
mod models;
mod query;
mod serde_helpers;
mod server;
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
mod settings;
#[cfg(feature = "desktop")]
mod tray;

use clap::Parser;
use config::Config;
use db::DbPool;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::LocalSessionManager,
};
use server::SteleServer;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Shared state for live rebinding the server to a new address.
pub struct BindState {
    pub bind_addr: std::sync::RwLock<String>,
    pub rebind_signal: tokio::sync::Notify,
}

/// Start the axum + MCP server. Loops to support live rebinding.
async fn run_server(
    config: Config,
    pool: DbPool,
    ct: CancellationToken,
    bind_state: Arc<BindState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let bind_addr = bind_state.bind_addr.read().unwrap().clone();

        let mcp_pool = pool.clone();
        let child_ct = ct.child_token();
        let service = StreamableHttpService::new(
            move || Ok(SteleServer::new(mcp_pool.clone())),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig {
                stateful_mode: true,
                sse_keep_alive: None,
                sse_retry: None,
                json_response: false,
                cancellation_token: child_ct.clone(),
            },
        );

        let mcp_path = config.mcp_path.clone();
        let app = axum::Router::new()
            .nest_service(&mcp_path, service)
            .nest("/api", api::router(pool.clone()));

        let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind to {bind_addr}: {e}");
                break;
            }
        };
        tracing::info!("Stele listening on {}", bind_addr);

        let serve_ct = child_ct.clone();
        let serve_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    serve_ct.cancelled().await;
                })
                .await
        });

        // Wait for shutdown, rebind signal, or server exit
        enum Action { Shutdown, Rebind, ServerDone(Result<Result<(), std::io::Error>, tokio::task::JoinError>) }
        let action = tokio::select! {
            _ = ct.cancelled() => Action::Shutdown,
            _ = bind_state.rebind_signal.notified() => Action::Rebind,
            result = serve_handle => Action::ServerDone(result),
        };

        match action {
            Action::Shutdown => {
                child_ct.cancel();
                break;
            }
            Action::Rebind => {
                let new_addr = bind_state.bind_addr.read().unwrap().clone();
                tracing::info!("Rebinding to {new_addr}");
                child_ct.cancel();
                continue;
            }
            Action::ServerDone(result) => {
                if let Ok(Err(e)) = result {
                    tracing::error!("Server error: {e}");
                }
                break;
            }
        }
    }

    Ok(())
}

/// Build a browser-friendly dashboard URL from a bind address.
#[cfg(feature = "desktop")]
pub fn dashboard_url(bind_addr: &str) -> String {
    format!(
        "http://{}/api/v1/stats",
        bind_addr.replace("0.0.0.0", "127.0.0.1")
    )
}

// ── Desktop mode (default): menu bar app, server on background thread ──

#[cfg(feature = "desktop")]
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::parse().with_desktop_defaults();

    // If --settings flag, run the egui dialog and exit
    if config.settings {
        if let Err(e) = settings::run_settings_dialog(&config.db) {
            tracing::error!("Settings dialog failed: {e}");
        }
        return;
    }

    tracing::info!(bind = %config.bind, db = %config.db, mcp_path = %config.mcp_path, "Starting Stele (desktop)");

    let pool = db::init_db(&config.db).expect("Failed to initialize database");
    let ct = CancellationToken::new();

    // Load persisted bind IP from config.toml (only if user didn't override via CLI/env)
    let bind_addr = {
        let config_path = settings::settings_path(&config.db);
        let saved = settings::load_settings(&config_path);
        if config.bind == "127.0.0.1:3100" {
            // User didn't override — apply saved IP with existing port
            let port = config.bind.rsplit_once(':').map(|(_, p)| p).unwrap_or("3100");
            format!("{}:{}", saved.server.bind_ip, port)
        } else {
            config.bind.clone()
        }
    };

    let bind_state = Arc::new(BindState {
        bind_addr: std::sync::RwLock::new(bind_addr.clone()),
        rebind_signal: tokio::sync::Notify::new(),
    });

    // Spawn the server on a background thread with its own tokio runtime
    let server_config = config.clone();
    let server_pool = pool.clone();
    let server_ct = ct.clone();
    let server_bind_state = bind_state.clone();
    let server_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        if let Err(e) = rt.block_on(run_server(server_config, server_pool, server_ct, server_bind_state)) {
            tracing::error!("Server error: {e}");
        }
    });

    // Run tray event loop on main thread (required by macOS and Windows)
    if let Err(e) = tray::run(ct.clone(), &bind_addr, bind_state, &config.db) {
        tracing::error!("Tray app error: {e}");
        // Fall back to waiting for Ctrl+C
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            tokio::signal::ctrl_c().await.ok();
            ct.cancel();
        });
    }

    let _ = server_handle.join();
    tracing::info!("Stele stopped");
}

// ── Headless mode: traditional async main, Ctrl+C shutdown ──

#[cfg(not(feature = "desktop"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::parse();

    tracing::info!(bind = %config.bind, db = %config.db, mcp_path = %config.mcp_path, "Starting Stele (headless)");

    let pool = db::init_db(&config.db)?;
    let ct = CancellationToken::new();

    let bind_state = Arc::new(BindState {
        bind_addr: std::sync::RwLock::new(config.bind.clone()),
        rebind_signal: tokio::sync::Notify::new(),
    });

    let shutdown_ct = ct.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("Ctrl+C received");
        shutdown_ct.cancel();
    });

    run_server(config, pool, ct, bind_state).await?;

    Ok(())
}
