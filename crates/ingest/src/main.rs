use axum::routing::{get, post, put};
use axum::Router;
use cylindersense_ingest::config::AppConfig;
use cylindersense_ingest::db;
use cylindersense_ingest::routes;
use sqlx::migrate::Migrator;
use std::path::Path;
use std::process::exit;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // ── Configuration ────────────────────────────────────────────────
    let config = AppConfig::from_env();

    // ── Tracing & Structured Logging ─────────────────────────────────
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    if config.log_format.eq_ignore_ascii_case("json") {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    tracing::info!(
        host = %config.host,
        port = %config.port,
        log_format = %config.log_format,
        "starting CylinderSense ingest service"
    );

    // ── Database ─────────────────────────────────────────────────────
    let pool = match db::create_pool(&config.database_url).await {
        Ok(p) => p,
        Err(err) => {
            eprintln!(
                "\n❌ DATABASE ERROR: Failed to connect to PostgreSQL at '{}'",
                config.database_url
            );
            eprintln!("   Details: {}\n", err);
            eprintln!("💡 Quickfix instructions:");
            eprintln!("   1. Make sure Docker Desktop / Postgres daemon is running.");
            eprintln!("   2. Re-create the local database volume:");
            eprintln!("      docker compose down -v");
            eprintln!("      docker compose up -d");
            eprintln!("   3. Or set DATABASE_URL to your PostgreSQL connection string:\n");
            eprintln!("      $env:DATABASE_URL=\"postgres://cs_dev:cs_dev_pass@localhost:5432/cylindersense\"\n");
            exit(1);
        }
    };
    tracing::info!("connected to database");

    // Run migrations from the `migrations/` directory next to the crate root.
    let migrator = Migrator::new(Path::new("crates/ingest/migrations"))
        .await
        .expect("failed to load migrations");
    if let Err(err) = migrator.run(&pool).await {
        eprintln!("\n❌ MIGRATION ERROR: Failed to execute database migrations.");
        eprintln!("   Details: {}\n", err);
        exit(1);
    }
    tracing::info!("database migrations applied");

    // ── Router & Web Service ─────────────────────────────────────────
    let app = Router::new()
        .route("/health", get(routes::health::health_check))
        .route(
            "/api/v1/telemetry",
            post(routes::telemetry::ingest_telemetry),
        )
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
        .route("/api/v1/refills/{id}", put(routes::refills::update_refill))
        .route("/api/v1/alerts", get(routes::alerts::list_alerts))
        .route(
            "/api/v1/alerts/{id}/acknowledge",
            post(routes::alerts::acknowledge_alert),
        )
        .layer(TraceLayer::new_for_http())
        .fallback_service(ServeDir::new("web"))
        .with_state(pool);

    // ── Server ───────────────────────────────────────────────────────
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .expect("failed to bind TCP listener");
    tracing::info!("listening on http://{}:{}", config.host, config.port);
    axum::serve(listener, app).await.expect("server error");
}
