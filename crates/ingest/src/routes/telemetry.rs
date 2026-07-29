use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::models::CylinderStatus;
use cylindersense_core::payloads::TelemetryPayload;
use cylindersense_engine::alert_rules::{generate_alert_message, should_trigger_alert};
use cylindersense_engine::state_estimator::{
    compute_gas_remaining, smooth_raw_readings, DEFAULT_FILL_GRAMS, DEFAULT_TARE_GRAMS,
};
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn parse_cylinder_status(s: &str) -> CylinderStatus {
    match s {
        "normal" => CylinderStatus::Normal,
        "low" => CylinderStatus::Low,
        "critical" => CylinderStatus::Critical,
        "offline" => CylinderStatus::Offline,
        _ => CylinderStatus::Unknown,
    }
}

/// POST /api/v1/telemetry
///
/// Validates incoming telemetry reading, persists it into the `telemetry_raw` table,
/// triggers the state engine to recompute remaining gas and cylinder status,
/// detects state transitions to trigger alert events, and updates `current_state`.
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

    // ── State Engine Re-evaluation & Alerting ──────────────────────────────
    update_derived_state_and_alert(&pool, &payload.device_id, payload.timestamp).await?;

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

/// Computes derived state for a device, evaluates alert transition rules,
/// inserts alert events when triggered, and updates `current_state`.
async fn update_derived_state_and_alert(
    pool: &PgPool,
    device_id: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Result<(), AppError> {
    // 1. Query current state to get previous_status before overwriting
    let prev_state_row = sqlx::query(
        r#"
        SELECT status
        FROM current_state
        WHERE device_id = $1
        "#,
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await?;

    let previous_status = match prev_state_row {
        Some(row) => parse_cylinder_status(row.get::<&str, _>("status")),
        None => CylinderStatus::Unknown,
    };

    // 2. Fetch active refill record (if any)
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

    // 3. Fetch recent raw telemetry readings for smoothing (last 10)
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

    // 4. Smooth raw load cell readings and compute derived gas state
    let smoothed_load = smooth_raw_readings(&readings, tare_grams, fill_amount_grams);
    let (remaining_grams, new_status) =
        compute_gas_remaining(smoothed_load, tare_grams, fill_amount_grams);

    // 5. Evaluate alert transition rules (Deduplication: triggers ONLY on new transition)
    if should_trigger_alert(previous_status, new_status) {
        let alert_id = Uuid::new_v4();
        let message = generate_alert_message(device_id, previous_status, new_status);

        sqlx::query(
            r#"
            INSERT INTO alert_events (id, device_id, state_from, state_to, triggered_at, message)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(alert_id)
        .bind(device_id)
        .bind(previous_status.to_string())
        .bind(new_status.to_string())
        .bind(timestamp)
        .bind(&message)
        .execute(pool)
        .await?;

        tracing::warn!(
            id = %alert_id,
            device_id = %device_id,
            from = %previous_status,
            to = %new_status,
            message = %message,
            "ALERT EVENT TRIGGERED"
        );
    }

    // 6. Upsert into current_state
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
    .bind(new_status.to_string())
    .bind(timestamp)
    .bind(active_refill_id)
    .execute(pool)
    .await?;

    tracing::info!(
        device_id = %device_id,
        remaining_grams = remaining_grams,
        status = %new_status,
        "updated derived current state"
    );

    Ok(())
}
