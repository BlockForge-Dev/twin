use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use sqlx::PgPool;

/// GET /health
///
/// Returns 200 OK with `status: healthy` when database connectivity is verified,
/// or 503 Service Unavailable with `status: unhealthy` if database ping fails.
pub async fn health_check(State(pool): State<PgPool>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "healthy",
                "database": "connected",
                "version": env!("CARGO_PKG_VERSION")
            })),
        ),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "unhealthy",
                "database": "disconnected",
                "error": err.to_string(),
                "version": env!("CARGO_PKG_VERSION")
            })),
        ),
    }
}
