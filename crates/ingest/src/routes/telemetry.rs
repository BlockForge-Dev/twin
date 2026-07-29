use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::payloads::TelemetryPayload;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// POST /api/v1/telemetry
///
/// Validates incoming telemetry reading, persists it into the `telemetry_raw` table,
/// and returns 201 Created with the saved record ID.
pub async fn ingest_telemetry(
    State(pool): State<PgPool>,
    Json(payload): Json<TelemetryPayload>,
) -> Result<impl IntoResponse, AppError> {
    // ── Validation ────────────────────────────────────────────────────────
    if payload.device_id.trim().is_empty() {
        return Err(AppError::Validation(
            "device_id cannot be empty or whitespace".to_string(),
        ));
    }

    if payload.raw_load_grams < 0 {
        return Err(AppError::Validation(format!(
            "raw_load_grams must be non-negative, got {}",
            payload.raw_load_grams
        )));
    }

    // ── Persistence ───────────────────────────────────────────────────────
    let id = Uuid::new_v4();
    let created_at = chrono::Utc::now();

    sqlx::query(
        r#"
        INSERT INTO telemetry_raw (id, device_id, timestamp, raw_load_grams, created_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(&payload.device_id)
    .bind(payload.timestamp)
    .bind(payload.raw_load_grams)
    .bind(created_at)
    .execute(&pool)
    .await?;

    tracing::info!(
        id = %id,
        device_id = %payload.device_id,
        timestamp = %payload.timestamp,
        raw_load_grams = payload.raw_load_grams,
        "persisted raw telemetry reading"
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "status": "stored",
            "id": id,
            "device_id": payload.device_id,
            "timestamp": payload.timestamp,
            "raw_load_grams": payload.raw_load_grams
        })),
    ))
}
