use axum::routing::{get, post, put};
use axum::Router;
use cylindersense_ingest::config::AppConfig;
use cylindersense_ingest::db;
use cylindersense_ingest::routes;
use sqlx::migrate::Migrator;
use std::path::Path;
use tower_http::services::ServeDir;
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
    let config = AppConfig::from_env().expect("failed to load config from environment");
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

    // ── Router & Web Service ─────────────────────────────────────────
    let app = Router::new()
        .route("/health", get(routes::health::health_check))
        .route("/api/v1/telemetry", post(routes::telemetry::ingest_telemetry))
        .route(
            "/api/v1/devices",
            post(routes::devices::register_device).get(routes::devices::list_devices),
        )
        .route(
            "/api/v1/devices/{id}/assign",
            post(routes::devices::assign_device),
        )
        .route(
            "/api/v1/devices/{id}/reassign",
            post(routes::devices::reassign_device),
        )
        .route(
            "/api/v1/devices/{id}/state",
            get(routes::state::get_device_state),
        )
        .route(
            "/api/v1/devices/{id}/refill",
            post(routes::refills::create_refill),
        )
        .route(
            "/api/v1/devices/{id}/refills",
            get(routes::refills::list_device_refills),
        )
        .route(
            "/api/v1/refills/{id}",
            put(routes::refills::update_refill),
        )
        .route("/api/v1/alerts", get(routes::alerts::list_alerts))
        .route(
            "/api/v1/alerts/{id}/acknowledge",
            post(routes::alerts::acknowledge_alert),
        )
        .fallback_service(ServeDir::new("web"))
        .with_state(pool);

    // ── Server ───────────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .expect("failed to bind TCP listener");
    tracing::info!("listening on http://{}:{}", config.host, config.port);
    axum::serve(listener, app)
        .await
        .expect("server error");
}
