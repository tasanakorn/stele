mod api;
mod config;
mod db;
mod models;
mod query;
mod server;
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

/// Start the axum + MCP server. Runs until the CancellationToken is cancelled.
async fn run_server(
    config: Config,
    pool: DbPool,
    ct: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mcp_pool = pool.clone();
    let service = StreamableHttpService::new(
        move || Ok(SteleServer::new(mcp_pool.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: None,
            sse_retry: None,
            json_response: false,
            cancellation_token: ct.clone(),
        },
    );

    let mcp_path = config.mcp_path.clone();
    let app = axum::Router::new()
        .nest_service(&mcp_path, service)
        .nest("/api", api::router(pool.clone()));

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("Stele listening on {}", config.bind);

    let server_ct = ct.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            server_ct.cancelled().await;
            tracing::info!("Shutting down server");
        })
        .await?;

    Ok(())
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

    tracing::info!(bind = %config.bind, db = %config.db, mcp_path = %config.mcp_path, "Starting Stele (desktop)");

    let pool = db::init_db(&config.db).expect("Failed to initialize database");
    let ct = CancellationToken::new();

    // Spawn the server on a background thread with its own tokio runtime
    let server_config = config.clone();
    let server_pool = pool.clone();
    let server_ct = ct.clone();
    let server_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        if let Err(e) = rt.block_on(run_server(server_config, server_pool, server_ct)) {
            tracing::error!("Server error: {e}");
        }
    });

    // Run tray event loop on main thread (required by macOS and Windows)
    if let Err(e) = tray::run(ct.clone(), &config.bind) {
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

    let shutdown_ct = ct.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        tracing::info!("Ctrl+C received");
        shutdown_ct.cancel();
    });

    run_server(config, pool, ct).await?;

    Ok(())
}
