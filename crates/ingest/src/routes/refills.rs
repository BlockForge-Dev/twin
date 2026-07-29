use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use cylindersense_core::error::AppError;
use cylindersense_core::models::RefillRecord;
use cylindersense_core::payloads::{CreateRefillPayload, UpdateRefillPayload};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::routes::telemetry::update_derived_state_and_alert;

#[derive(Debug, sqlx::FromRow)]
struct RefillRecordRow {
    id: Uuid,
    device_id: String,
    fill_amount_grams: i32,
    cylinder_name: Option<String>,
    cylinder_profile: Option<String>,
    refill_date: chrono::DateTime<chrono::Utc>,
    edited_by: Option<String>,
    notes: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<RefillRecordRow> for RefillRecord {
    fn from(r: RefillRecordRow) -> Self {
        RefillRecord {
            id: r.id,
            device_id: r.device_id,
            fill_amount_grams: r.fill_amount_grams,
            cylinder_name: r.cylinder_name,
            cylinder_profile: r.cylinder_profile,
            refill_date: r.refill_date,
            edited_by: r.edited_by,
            notes: r.notes,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// POST /api/v1/devices/{id}/refill
///
/// Records a new cylinder refill event for a device, sets it active,
/// and triggers immediate state recomputation (gas remaining jumps to full).
pub async fn create_refill(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
    Json(payload): Json<CreateRefillPayload>,
) -> Result<impl IntoResponse, AppError> {
    if payload.fill_amount_kg <= 0.0 {
        return Err(AppError::Validation(format!(
            "fill_amount_kg must be greater than 0, got {}",
            payload.fill_amount_kg
        )));
    }

    let fill_grams = (payload.fill_amount_kg * 1000.0).round() as i32;
    let trimmed_id = id_param.trim();
    let parsed_uuid = Uuid::parse_str(trimmed_id).ok();

    // Resolve device_id string (either directly or via UUID primary key)
    let dev_row = sqlx::query("SELECT device_id FROM devices WHERE device_id = $1 OR id = $2")
        .bind(trimmed_id)
        .bind(parsed_uuid)
        .fetch_optional(&pool)
        .await?;

    let device_id = match dev_row {
        Some(row) => row.get::<String, _>("device_id"),
        None => trimmed_id.to_string(), // fallback to provided string
    };

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let record = sqlx::query_as::<_, RefillRecordRow>(
        r#"
        INSERT INTO refill_records (id, device_id, fill_amount_grams, cylinder_name, cylinder_profile, refill_date, edited_by, notes, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $6, $6)
        RETURNING id, device_id, fill_amount_grams, cylinder_name, cylinder_profile, refill_date, edited_by, notes, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(&device_id)
    .bind(fill_grams)
    .bind(&payload.cylinder_name)
    .bind(&payload.cylinder_profile)
    .bind(now)
    .bind(&payload.edited_by)
    .bind(&payload.notes)
    .fetch_one(&pool)
    .await?;

    let refill: RefillRecord = record.into();

    tracing::info!(
        refill_id = %refill.id,
        device_id = %device_id,
        fill_kg = payload.fill_amount_kg,
        edited_by = ?payload.edited_by,
        "recorded new refill event — recalculating state"
    );

    // Immediately trigger state re-evaluation so remaining gas jumps to full
    update_derived_state_and_alert(&pool, &device_id, now).await?;

    Ok((StatusCode::CREATED, Json(refill)))
}

/// PUT /api/v1/refills/{id}
///
/// Edits an existing refill record, updates audit fields, and recalculates device state.
pub async fn update_refill(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
    Json(payload): Json<UpdateRefillPayload>,
) -> Result<impl IntoResponse, AppError> {
    let refill_id = Uuid::parse_str(id_param.trim())
        .map_err(|_| AppError::Validation(format!("invalid refill UUID: {}", id_param)))?;

    let new_fill_grams = payload
        .fill_amount_kg
        .map(|kg| (kg * 1000.0).round() as i32);

    let now = chrono::Utc::now();

    let record = sqlx::query_as::<_, RefillRecordRow>(
        r#"
        UPDATE refill_records
        SET fill_amount_grams = COALESCE($1, fill_amount_grams),
            cylinder_name     = COALESCE($2, cylinder_name),
            cylinder_profile  = COALESCE($3, cylinder_profile),
            edited_by         = COALESCE($4, edited_by),
            notes             = COALESCE($5, notes),
            updated_at        = $6
        WHERE id = $7
        RETURNING id, device_id, fill_amount_grams, cylinder_name, cylinder_profile, refill_date, edited_by, notes, created_at, updated_at
        "#,
    )
    .bind(new_fill_grams)
    .bind(&payload.cylinder_name)
    .bind(&payload.cylinder_profile)
    .bind(&payload.edited_by)
    .bind(&payload.notes)
    .bind(now)
    .bind(refill_id)
    .fetch_optional(&pool)
    .await?;

    match record {
        Some(r) => {
            let refill: RefillRecord = r.into();

            tracing::info!(
                refill_id = %refill.id,
                device_id = %refill.device_id,
                new_fill_grams = refill.fill_amount_grams,
                edited_by = ?refill.edited_by,
                "updated refill record — recalculating state"
            );

            // Recalculate state based on updated refill parameters
            update_derived_state_and_alert(&pool, &refill.device_id, now).await?;

            Ok((StatusCode::OK, Json(refill)))
        }
        None => Err(AppError::NotFound(format!(
            "refill record not found: {}",
            refill_id
        ))),
    }
}

/// GET /api/v1/devices/{id}/refills
///
/// Returns audit history of all refill records for a device ordered by date.
pub async fn list_device_refills(
    State(pool): State<PgPool>,
    Path(id_param): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let trimmed_id = id_param.trim();
    let parsed_uuid = Uuid::parse_str(trimmed_id).ok();

    let records = sqlx::query_as::<_, RefillRecordRow>(
        r#"
        SELECT id, device_id, fill_amount_grams, cylinder_name, cylinder_profile, refill_date, edited_by, notes, created_at, updated_at
        FROM refill_records
        WHERE device_id = $1 OR device_id = (SELECT device_id FROM devices WHERE id = $2)
        ORDER BY refill_date DESC
        "#,
    )
    .bind(trimmed_id)
    .bind(parsed_uuid)
    .fetch_all(&pool)
    .await?;

    let refills: Vec<RefillRecord> = records.into_iter().map(Into::into).collect();
    Ok((StatusCode::OK, Json(refills)))
}
