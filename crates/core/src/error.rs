use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

/// Application-level error type shared across the workspace.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = Json(json!({
            "error": message,
            "type": format!("{:?}", self)
        }));

        (status, body).into_response()
    }
}
