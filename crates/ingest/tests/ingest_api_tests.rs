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

#[tokio::test]
async fn test_health_check_endpoint() {
    let pool = dummy_pool().await;
    let app = build_router(pool);

    let req = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}


// ── Integration Tests (Requires Postgres) ────────────────────────────────────

#[tokio::test]
async fn test_db_integration_operator_workflows_refill_edit_and_audit() {
    let Some(pool) = setup_test_pool().await else {
        eprintln!("Skipping DB integration test: DATABASE_URL not available or DB unreachable");
        return;
    };

    let test_device_id = format!("test-dev-{}", uuid::Uuid::new_v4());

    // 1. Register device
    let reg_payload = json!({ "device_id": test_device_id });
    let (status, _) = execute_post(&pool, "/api/v1/devices", reg_payload).await;
    assert_eq!(status, StatusCode::CREATED);

    // 2. Ingest low level telemetry (7500g raw load -> 2000g remaining gas -> Low state)
    let tel_low = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:00:00Z",
        "raw_load_grams": 7500
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_low).await;
    assert_eq!(status, StatusCode::CREATED);

    let state_url = format!("/api/v1/devices/{}/state", test_device_id);
    let (status, body) = execute_get(&pool, &state_url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "low");

    // 3. Record a Refill: POST /api/v1/devices/{id}/refill (12.5kg fill)
    let refill_payload = json!({
        "fill_amount_kg": 12.5,
        "cylinder_name": "Kitchen Main Tank",
        "cylinder_profile": "12.5kg",
        "edited_by": "operator-john",
        "notes": "Replaced empty tank with fresh 12.5kg cylinder"
    });

    let refill_url = format!("/api/v1/devices/{}/refill", test_device_id);
    let (status, refill_body) = execute_post(&pool, &refill_url, refill_payload).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(refill_body["fill_amount_grams"], 12500);
    assert_eq!(refill_body["edited_by"], "operator-john");
    let refill_id = refill_body["id"].as_str().unwrap();

    // 4. Ingest post-refill reading (18000g raw load = 5500g tare + 12500g gas)
    let tel_full = json!({
        "device_id": test_device_id,
        "timestamp": "2026-07-29T14:05:00Z",
        "raw_load_grams": 18000
    });
    let (status, _) = execute_post(&pool, "/api/v1/telemetry", tel_full).await;
    assert_eq!(status, StatusCode::CREATED);

    // Assert state recalculated: remaining gas jumps to full (12500g) and status becomes Normal
    let (status, body) = execute_get(&pool, &state_url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "normal");
    assert_eq!(body["remaining_grams"], 12500);

    // 5. Edit Refill: PUT /api/v1/refills/{id} (Correct amount to 6.0kg fill)
    let edit_payload = json!({
        "fill_amount_kg": 6.0,
        "edited_by": "manager-alice",
        "notes": "Corrected fill amount to 6kg"
    });
    let edit_url = format!("/api/v1/refills/{}", refill_id);
    let (status, edit_body) = execute_put(&pool, &edit_url, edit_payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edit_body["fill_amount_grams"], 6000);
    assert_eq!(edit_body["edited_by"], "manager-alice");

    // Assert state recalculated after edit: remaining gas clamped to 6000g
    let (status, body) = execute_get(&pool, &state_url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["remaining_grams"], 6000);

    // 6. Query Refill Audit History: GET /api/v1/devices/{id}/refills
    let history_url = format!("/api/v1/devices/{}/refills", test_device_id);
    let (status, history_body) = execute_get(&pool, &history_url).await;
    assert_eq!(status, StatusCode::OK);
    let refills = history_body.as_array().expect("array of refills");
    assert_eq!(refills.len(), 1);
    assert_eq!(refills[0]["edited_by"], "manager-alice");

    // 7. Reassign Device: POST /api/v1/devices/{id}/reassign
    let reassign_payload = json!({
        "site_id": "bakery-site-2",
        "edited_by": "op-john"
    });
    let reassign_url = format!("/api/v1/devices/{}/reassign", test_device_id);
    let (status, reassign_body) = execute_post(&pool, &reassign_url, reassign_payload).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reassign_body["site_id"], "bakery-site-2");
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

async fn execute_put(pool: &PgPool, uri: &str, payload: Value) -> (StatusCode, Value) {
    let app = build_router(pool.clone());

    let req = Request::builder()
        .method("PUT")
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
            "/api/v1/devices/{id}/reassign",
            axum::routing::post(cylindersense_ingest_routes::reassign_device),
        )
        .route(
            "/api/v1/devices/{id}/state",
            axum::routing::get(cylindersense_ingest_routes::get_device_state),
        )
        .route(
            "/api/v1/devices/{id}/refill",
            axum::routing::post(cylindersense_ingest_routes::create_refill),
        )
        .route(
            "/api/v1/devices/{id}/refills",
            axum::routing::get(cylindersense_ingest_routes::list_device_refills),
        )
        .route(
            "/api/v1/refills/{id}",
            axum::routing::put(cylindersense_ingest_routes::update_refill),
        )
        .route("/api/v1/alerts", axum::routing::get(cylindersense_ingest_routes::list_alerts))
        .route(
            "/api/v1/alerts/{id}/acknowledge",
            axum::routing::post(cylindersense_ingest_routes::acknowledge_alert),
        )
        .fallback_service(tower_http::services::ServeDir::new("../../web"))
        .with_state(pool)
}

mod cylindersense_ingest_routes {
    pub use cylindersense_ingest::routes::alerts::{acknowledge_alert, list_alerts};
    pub use cylindersense_ingest::routes::devices::{assign_device, list_devices, reassign_device, register_device};
    pub use cylindersense_ingest::routes::health::health_check as health;
    pub use cylindersense_ingest::routes::refills::{create_refill, list_device_refills, update_refill};
    pub use cylindersense_ingest::routes::state::get_device_state;
    pub use cylindersense_ingest::routes::telemetry::ingest_telemetry as telemetry;
}
