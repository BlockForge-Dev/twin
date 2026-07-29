use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::payloads::TelemetryPayload;
use cylindersense_engine::state_estimator::{
    compute_gas_remaining, smooth_raw_readings, DEFAULT_FILL_GRAMS, DEFAULT_TARE_GRAMS,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// POST /api/v1/telemetry
///
/// Validates incoming telemetry reading, persists it into the `telemetry_raw` table,
/// triggers the state engine to recompute remaining gas and cylinder status,
/// and updates `current_state`.
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

    // ── Persistence to telemetry_raw ──────────────────────────────────────
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

    // ── State Engine Re-evaluation ─────────────────────────────────────────
    update_derived_state(&pool, &payload.device_id, payload.timestamp).await?;

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

/// Computes derived state for a device and updates `current_state`.
async fn update_derived_state(
    pool: &PgPool,
    device_id: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    // 1. Fetch active refill record (if any)
    let refill_row = sqlx::query(
        r#"
        SELECT id, fill_amount_grams
        FROM refill_records
        WHERE device_id = $1
        ORDER BY refill_date DESC
        LIMIT 1
        "#,
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;

    let (active_refill_id, fill_amount_grams, tare_grams) = match refill_row {
        Some(row) => (
            Some(row.get::<Uuid, _>("id")),
            row.get::<i32, _>("fill_amount_grams"),
            DEFAULT_TARE_GRAMS,
        ),
        None => (None, DEFAULT_FILL_GRAMS, DEFAULT_TARE_GRAMS),
    };

    // 2. Fetch recent raw telemetry readings for smoothing (last 10)
    let rows = sqlx::query(
        r#"
        SELECT raw_load_grams
        FROM telemetry_raw
        WHERE device_id = $1
        ORDER BY timestamp DESC
        LIMIT 10
        "#,
    )
    .bind(device_id)
    .fetch_all(pool)
    .await?;

    let readings: Vec<i32> = rows.iter().map(|r| r.get("raw_load_grams")).collect();

    // 3. Smooth raw load cell readings and compute derived gas state
    let smoothed_load = smooth_raw_readings(&readings, tare_grams, fill_amount_grams);
    let (remaining_grams, status) =
        compute_gas_remaining(smoothed_load, tare_grams, fill_amount_grams);

    // 4. Upsert into current_state
    sqlx::query(
        r#"
        INSERT INTO current_state (device_id, remaining_grams, status, last_seen_at, active_refill_id, updated_at)
        VALUES ($1, $2, $3, $4, $5, NOW())
        ON CONFLICT (device_id) DO UPDATE SET
            remaining_grams = EXCLUDED.remaining_grams,
            status = EXCLUDED.status,
            last_seen_at = EXCLUDED.last_seen_at,
            active_refill_id = EXCLUDED.active_refill_id,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(device_id)
    .bind(remaining_grams)
    .bind(status.to_string())
    .bind(timestamp)
    .bind(active_refill_id)
    .execute(pool)
    .await?;

    tracing::info!(
        device_id = %device_id,
        remaining_grams = remaining_grams,
        status = %status,
        "updated derived current state"
    );

    Ok(())
}
