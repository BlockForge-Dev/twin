use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::models::{AlertEvent, CylinderStatus};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AlertQuery {
    pub device_id: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct AlertEventRow {
    id: Uuid,
    device_id: String,
    state_from: String,
    state_to: String,
    triggered_at: chrono::DateTime<chrono::Utc>,
    acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
    message: Option<String>,
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

impl From<AlertEventRow> for AlertEvent {
    fn from(r: AlertEventRow) -> Self {
        AlertEvent {
            id: r.id,
            device_id: r.device_id,
            state_from: parse_cylinder_status(&r.state_from),
            state_to: parse_cylinder_status(&r.state_to),
            triggered_at: r.triggered_at,
            acknowledged_at: r.acknowledged_at,
            message: r.message,
        }
    }
}

/// GET /api/v1/alerts
///
/// Returns a list of alert events, optionally filtered by `device_id`.
pub async fn list_alerts(
    State(pool): State<PgPool>,
    Query(query): Query<AlertQuery>,
) -> Result<impl IntoResponse, AppError> {
    let records = if let Some(device_id) = &query.device_id {
        sqlx::query_as::<_, AlertEventRow>(
            r#"
            SELECT id, device_id, state_from, state_to, triggered_at, acknowledged_at, message
            FROM alert_events
            WHERE device_id = $1
            ORDER BY triggered_at DESC
            "#,
        )
        .bind(device_id.trim())
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, AlertEventRow>(
            r#"
            SELECT id, device_id, state_from, state_to, triggered_at, acknowledged_at, message
            FROM alert_events
            ORDER BY triggered_at DESC
            "#,
        )
        .fetch_all(&pool)
        .await?
    };

    let alerts: Vec<AlertEvent> = records.into_iter().map(Into::into).collect();
    Ok((StatusCode::OK, Json(alerts)))
}

/// POST /api/v1/alerts/{id}/acknowledge
///
/// Marks an alert event as acknowledged by an operator.
pub async fn acknowledge_alert(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let alert_id = Uuid::parse_str(id_param.trim()).map_err(|_| {
        AppError::Validation(format!("invalid alert UUID: {}", id_param))
    })?;

    let now = chrono::Utc::now();

    let record = sqlx::query_as::<_, AlertEventRow>(
        r#"
        UPDATE alert_events
        SET acknowledged_at = $1
        WHERE id = $2
        RETURNING id, device_id, state_from, state_to, triggered_at, acknowledged_at, message
        "#,
    )
    .bind(now)
    .bind(alert_id)
    .fetch_optional(&pool)
    .await?;

    match record {
        Some(r) => {
            let alert: AlertEvent = r.into();
            tracing::info!(id = %alert.id, device_id = %alert.device_id, "alert acknowledged");
            Ok((StatusCode::OK, Json(alert)))
        }
        None => Err(AppError::NotFound(format!(
            "alert event not found: {}",
            alert_id
        ))),
    }
}
