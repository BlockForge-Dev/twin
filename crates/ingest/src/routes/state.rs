use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::models::{CurrentState, CylinderStatus};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
struct CurrentStateRow {
    device_id: String,
    remaining_grams: Option<i32>,
    status: String,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
    active_refill_id: Option<Uuid>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

fn parse_cylinder_status(s: &str) -> CylinderStatus {
    match s {
        "normal" => CylinderStatus::Normal,
        "low" => CylinderStatus::Low,
        "critical" => CylinderStatus::Critical,
        "offline" => CylinderStatus::Offline,
        _ => CylinderStatus::Unknown,
    }
}

/// GET /api/v1/devices/{id}/state
///
/// Returns the latest derived operational state for a device.
/// `:id` can be either the hardware device_id string or internal UUID string.
pub async fn get_device_state(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let trimmed_id = id_param.trim();
    let parsed_uuid = Uuid::parse_str(trimmed_id).ok();

    // Query state by device_id or lookup via devices table ID
    let record = sqlx::query_as::<_, CurrentStateRow>(
        r#"
        SELECT s.device_id, s.remaining_grams, s.status, s.last_seen_at, s.active_refill_id, s.updated_at
        FROM current_state s
        WHERE s.device_id = $1
           OR s.device_id = (SELECT device_id FROM devices WHERE id = $2)
        "#,
    )
    .bind(trimmed_id)
    .bind(parsed_uuid)
    .fetch_optional(&pool)
    .await?;

    match record {
        Some(r) => {
            let state = CurrentState {
                device_id: r.device_id,
                remaining_grams: r.remaining_grams,
                status: parse_cylinder_status(&r.status),
                last_seen_at: r.last_seen_at,
                active_refill_id: r.active_refill_id,
                updated_at: r.updated_at,
            };
            Ok((StatusCode::OK, Json(state)))
        }
        None => Err(AppError::NotFound(format!(
            "current state not found for device: {}",
            id_param
        ))),
    }
}
