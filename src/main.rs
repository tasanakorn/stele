mod config;
mod db;
mod models;
mod query;
mod server;

use clap::Parser;
use config::Config;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::LocalSessionManager,
};
use server::SteleServer;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::parse();

    tracing::info!(bind = %config.bind, db = %config.db, mcp_path = %config.mcp_path, "Starting Stele");

    let pool = db::init_db(&config.db)?;

    let ct = CancellationToken::new();

    let service = StreamableHttpService::new(
        move || Ok(SteleServer::new(pool.clone())),
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
    let app = axum::Router::new().nest_service(&mcp_path, service);

    let listener = TcpListener::bind(&config.bind).await?;
    tracing::info!("Stele listening on {}", config.bind);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            tracing::info!("Shutting down");
            ct.cancel();
        })
        .await?;

    Ok(())
}
