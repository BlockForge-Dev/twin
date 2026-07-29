use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::payloads::TelemetryPayload;
use serde_json::json;

/// POST /api/v1/telemetry
///
/// Stub handler for M1. Accepts the telemetry payload, logs it, and
/// returns 200 OK. Full ingestion logic (DB insert, state engine trigger)
/// will be implemented in M2.
pub async fn ingest_telemetry(
    Json(payload): Json<TelemetryPayload>,
) -> impl IntoResponse {
    tracing::info!(
        device_id = %payload.device_id,
        timestamp = %payload.timestamp,
        raw_load_grams = %payload.raw_load_grams,
        "received telemetry"
    );

    (StatusCode::OK, Json(json!({"status": "accepted"})))
}
