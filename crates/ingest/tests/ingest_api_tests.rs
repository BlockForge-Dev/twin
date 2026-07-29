use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::env;
use tower::ServiceExt;

// ── App builder helper for tests ─────────────────────────────────────────────

async fn setup_test_pool() -> Option<PgPool> {
    let db_url = env::var("DATABASE_URL").ok()?;
    let pool = sqlx::PgPool::connect(&db_url).await.ok()?;

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("migrations"))
        .await
        .ok()?;
    migrator.run(&pool).await.ok()?;

    Some(pool)
}

// ── Validation Unit Tests (Run without DB) ───────────────────────────────────

#[tokio::test]
async fn test_telemetry_validation_empty_device_id() {
    let payload = json!({
        "device_id": "   ",
        "timestamp": "2026-07-29T12:00:00Z",
        "raw_load_grams": 1000
    });

    let (status, body) = emulate_telemetry_request(payload).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("device_id cannot be empty"));
}

#[tokio::test]
async fn test_telemetry_validation_negative_load() {
    let payload = json!({
        "device_id": "test-device-001",
        "timestamp": "2026-07-29T12:00:00Z",
        "raw_load_grams": -50
    });

    let (status, body) = emulate_telemetry_request(payload).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("must be non-negative"));
}

#[tokio::test]
async fn test_assign_device_validation_empty_site_id() {
    let payload = json!({
        "site_id": "  "
    });

    let (status, body) = emulate_assign_request("nonexistent-device", payload).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("site_id cannot be empty"));
}

// ── Integration Tests (Requires Postgres) ────────────────────────────────────

#[tokio::test]
async fn test_db_integration_device_telemetry_state_and_alerts() {
    let Some(pool) = setup_test_pool().await else {
        eprintln!("Skipping DB integration test: DATABASE_URL not available or DB unreachable");
        return;
    };

    let test_device_id = format!("test-dev-{}", uuid::Uuid::new_v4());

    // 1. Register and assign device
    let reg_payload = json!({ "device_id": test_device_id });
    let (status, _) = execute_post(&pool, "/api/v1/devices", reg_payload).await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Ingest normal telemetry (15500g raw load -> Normal state)
    let tel_normal = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:00:00Z",
        "raw_load_grams": 15500
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_normal).await;
    assert_eq!(status, StatusCode::CREATED);

    // 3. Ingest low telemetry (7500g raw load -> Low state -> Triggers Alert #1: Normal -> Low)
    let tel_low_1 = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:01:00Z",
        "raw_load_grams": 7500
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_low_1).await;
    assert_eq!(status, StatusCode::CREATED);

    // 4. Ingest identical low telemetry (7500g raw load -> Low state -> Deduplicated, NO duplicate alert!)
    let tel_low_2 = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:02:00Z",
        "raw_load_grams": 7500
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_low_2).await;
    assert_eq!(status, StatusCode::CREATED);

    // 5. Ingest critical telemetry (6000g raw load -> Critical state -> Triggers Alert #2: Low -> Critical)
    let tel_crit = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:03:00Z",
        "raw_load_grams": 6000
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_crit).await;
    assert_eq!(status, StatusCode::CREATED);

    // 6. Query GET /api/v1/alerts?device_id=... and assert exact alert events
    let alerts_url = format!("/api/v1/alerts?device_id={}", test_device_id);
    let (status, body) = execute_get(&pool, &alerts_url).await;
    assert_eq!(status, StatusCode::OK);

    let alerts = body.as_array().expect("expected JSON array of alerts");
    assert_eq!(alerts.len(), 2, "exactly 2 alert events should be triggered (Low & Critical)");

    let latest_alert = &alerts[0];
    assert_eq!(latest_alert["state_to"], "critical");
    assert_eq!(latest_alert["state_from"], "low");

    let prev_alert = &alerts[1];
    assert_eq!(prev_alert["state_to"], "low");
    assert_eq!(prev_alert["state_from"], "normal");

    // 7. Acknowledge alert via POST /api/v1/alerts/{id}/acknowledge
    let alert_id = latest_alert["id"].as_str().unwrap();
    let ack_url = format!("/api/v1/alerts/{}/acknowledge", alert_id);
    let (status, ack_body) = execute_post(&pool, &ack_url, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ack_body["acknowledged_at"].is_string(), "acknowledged_at timestamp should be set");
}

// ── Test Execution Helpers ───────────────────────────────────────────────────

async fn emulate_telemetry_request(payload: Value) -> (StatusCode, Value) {
    let pool = dummy_pool().await;
    let app = build_router(pool);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/telemetry")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body_json)
}

async fn emulate_assign_request(device_id: &str, payload: Value) -> (StatusCode, Value) {
    let pool = dummy_pool().await;
    let app = build_router(pool);

    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/devices/{}/assign", device_id))
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body_json)
}

async fn execute_post(pool: &PgPool, uri: &str, payload: Value) -> (StatusCode, Value) {
    let app = build_router(pool.clone());

    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body_json)
}

async fn execute_get(pool: &PgPool, uri: &str) -> (StatusCode, Value) {
    let app = build_router(pool.clone());

    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    (status, body_json)
}

async fn dummy_pool() -> PgPool {
    sqlx::PgPool::connect_lazy("postgres://localhost/dummy").unwrap()
}

fn build_router(pool: PgPool) -> axum::Router {
    axum::Router::new()
        .route("/health", axum::routing::get(cylindersense_ingest_routes::health))
        .route("/api/v1/telemetry", axum::routing::post(cylindersense_ingest_routes::telemetry))
        .route(
            "/api/v1/devices",
            axum::routing::post(cylindersense_ingest_routes::register_device).get(cylindersense_ingest_routes::list_devices),
        )
        .route(
            "/api/v1/devices/{id}/assign",
            axum::routing::post(cylindersense_ingest_routes::assign_device),
        )
        .route(
            "/api/v1/devices/{id}/state",
            axum::routing::get(cylindersense_ingest_routes::get_device_state),
        )
        .route("/api/v1/alerts", axum::routing::get(cylindersense_ingest_routes::list_alerts))
        .route(
            "/api/v1/alerts/{id}/acknowledge",
            axum::routing::post(cylindersense_ingest_routes::acknowledge_alert),
        )
        .with_state(pool)
}

mod cylindersense_ingest_routes {
    pub use cylindersense_ingest::routes::alerts::{acknowledge_alert, list_alerts};
    pub use cylindersense_ingest::routes::devices::{assign_device, list_devices, register_device};
    pub use cylindersense_ingest::routes::health::health_check as health;
    pub use cylindersense_ingest::routes::state::get_device_state;
    pub use cylindersense_ingest::routes::telemetry::ingest_telemetry as telemetry;
}
