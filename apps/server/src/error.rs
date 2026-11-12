//! Structured API errors. Every error response has the shape
//! `{ "error": { "code": "...", "message": "..." } }` with a safe message.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use nook_shared::ApiErrorEnvelope;

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self { status, code, message: message.into() }
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "Please log in to continue.")
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "validation", message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", message)
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let msg: String = message.into();
        tracing::error!("internal error: {msg}");
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "Something went wrong on our side. Please try again.")
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        use sqlx::Error;
        match &err {
            Error::RowNotFound => Self::not_found("Not found."),
            _ => Self::internal(format!("database error: {err}")),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiErrorEnvelope {
            error: nook_shared::ApiErrorBody {
                code: self.code.to_string(),
                message: self.message,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

pub type ApiResult<T> = Result<T, AppError>;
