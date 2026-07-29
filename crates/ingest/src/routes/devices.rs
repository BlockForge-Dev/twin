use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::models::{Device, DeviceStatus};
use cylindersense_core::payloads::{AssignDevicePayload, RegisterDevicePayload};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    device_id: String,
    model: Option<String>,
    firmware_version: Option<String>,
    status: String,
    site_id: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<DeviceRow> for Device {
    fn from(r: DeviceRow) -> Self {
        let status = match r.status.as_str() {
            "active" => DeviceStatus::Active,
            "inactive" => DeviceStatus::Inactive,
            _ => DeviceStatus::Uninitialized,
        };

        Device {
            id: r.id,
            device_id: r.device_id,
            model: r.model,
            firmware_version: r.firmware_version,
            status,
            site_id: r.site_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// POST /api/v1/devices
///
/// Registers a new physical monitoring unit in the database.
pub async fn register_device(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterDevicePayload>,
) -> Result<impl IntoResponse, AppError> {
    let trimmed_id = payload.device_id.trim();
    if trimmed_id.is_empty() {
        return Err(AppError::Validation(
            "device_id cannot be empty or whitespace".to_string(),
        ));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let initial_status = DeviceStatus::Uninitialized.to_string();

    let record = sqlx::query_as::<_, DeviceRow>(
        r#"
        INSERT INTO devices (id, device_id, model, firmware_version, status, site_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, NULL, $6, $6)
        ON CONFLICT (device_id) DO UPDATE SET
            model = EXCLUDED.model,
            firmware_version = EXCLUDED.firmware_version,
            updated_at = EXCLUDED.updated_at
        RETURNING id, device_id, model, firmware_version, status, site_id, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(trimmed_id)
    .bind(payload.model)
    .bind(payload.firmware_version)
    .bind(initial_status)
    .bind(now)
    .fetch_one(&pool)
    .await?;

    let device: Device = record.into();

    tracing::info!(
        device_id = %device.device_id,
        id = %device.id,
        "registered monitoring device"
    );

    Ok((StatusCode::CREATED, Json(device)))
}

/// GET /api/v1/devices
///
/// Returns a list of all registered monitoring devices.
pub async fn list_devices(
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, AppError> {
    let records = sqlx::query_as::<_, DeviceRow>(
        r#"
        SELECT id, device_id, model, firmware_version, status, site_id, created_at, updated_at
        FROM devices
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&pool)
    .await?;

    let devices: Vec<Device> = records.into_iter().map(Into::into).collect();

    Ok((StatusCode::OK, Json(devices)))
}

/// POST /api/v1/devices/:id/assign
///
/// Assigns a device to a site/location and updates its status to active.
/// `:id` can be either the hardware device_id string or internal UUID string.
pub async fn assign_device(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
    Json(payload): Json<AssignDevicePayload>,
) -> Result<impl IntoResponse, AppError> {
    let trimmed_site = payload.site_id.trim();
    if trimmed_site.is_empty() {
        return Err(AppError::Validation(
            "site_id cannot be empty or whitespace".to_string(),
        ));
    }

    let active_status = DeviceStatus::Active.to_string();
    let trimmed_id = id_param.trim();
    let parsed_uuid = Uuid::parse_str(trimmed_id).ok();

    let record = sqlx::query_as::<_, DeviceRow>(
        r#"
        UPDATE devices
        SET site_id = $1,
            status = $2,
            updated_at = NOW()
        WHERE device_id = $3 OR id = $4
        RETURNING id, device_id, model, firmware_version, status, site_id, created_at, updated_at
        "#,
    )
    .bind(trimmed_site)
    .bind(active_status)
    .bind(trimmed_id)
    .bind(parsed_uuid)
    .fetch_optional(&pool)
    .await?;

    match record {
        Some(r) => {
            let device: Device = r.into();

            tracing::info!(
                device_id = %device.device_id,
                site_id = %trimmed_site,
                "assigned device to site"
            );

            Ok((StatusCode::OK, Json(device)))
        }
        None => Err(AppError::NotFound(format!(
            "device not found: {}",
            id_param
        ))),
    }
}
