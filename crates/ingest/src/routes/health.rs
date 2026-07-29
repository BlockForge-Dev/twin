use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

/// GET /health
///
/// Returns 200 OK with a simple status payload.
/// Used for liveness probes and basic connectivity checks.
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}
