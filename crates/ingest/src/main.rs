mod config;
mod db;
mod routes;

use axum::Router;
use axum::routing::{get, post};
use sqlx::migrate::Migrator;
use std::path::Path;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // ── Tracing ──────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // ── Configuration ────────────────────────────────────────────────
    let config = config::AppConfig::from_env().expect("failed to load config from environment");
    tracing::info!(host = %config.host, port = %config.port, "starting CylinderSense ingest service");

    // ── Database ─────────────────────────────────────────────────────
    let pool = db::create_pool(&config.database_url)
        .await
        .expect("failed to connect to database");
    tracing::info!("connected to database");

    // Run migrations from the `migrations/` directory next to the crate root.
    let migrator = Migrator::new(Path::new("crates/ingest/migrations"))
        .await
        .expect("failed to load migrations");
    migrator
        .run(&pool)
        .await
        .expect("failed to run database migrations");
    tracing::info!("database migrations applied");

    // ── Router ───────────────────────────────────────────────────────
    let app = Router::new()
        .route("/health", get(routes::health::health_check))
        .route("/api/v1/telemetry", post(routes::telemetry::ingest_telemetry));

    // ── Server ───────────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .expect("failed to bind TCP listener");
    tracing::info!("listening on {}:{}", config.host, config.port);
    axum::serve(listener, app)
        .await
        .expect("server error");
}
