use thiserror::Error;
use axum::{http::StatusCode, response::{IntoResponse, Response}};

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Unauthorized")] 
    Unauthorized,
    #[error("Forbidden")] 
    Forbidden,
    #[error("Request rejected: API called from an unrecognized source.")]
    UnrecognizedSource,
    #[error("Too many requests")]
    TooManyRequests,
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal server error")]
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, details, should_report) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", None, false),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", None, false),
            AppError::UnrecognizedSource => (StatusCode::FORBIDDEN, "unrecognized_source", None, true), // Security issue
            AppError::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", None, false),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", Some(msg.clone()), false),
            AppError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal", None, true), // Always report internal errors
        };
        
        // Report critical errors to Sentry
        if should_report {
            sentry::capture_message(&self.to_string(), sentry::Level::Error);
        }
        
        // Return explicit technical error messages - transformation happens in client-server
        let mut body = serde_json::json!({
            "error": self.to_string(),
            "code": code
        });
        if let Some(details) = details {
            body["details"] = serde_json::Value::String(details);
        }
        (status, axum::Json(body)).into_response()
    }
}
