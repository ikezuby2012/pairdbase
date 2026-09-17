use crate::abstractions::ApiResponse;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("connection not found")]
    NotFound,

    #[error("unsupported database type: {0}")]
    UnsupportedDb(String),

    #[error("connection test failed: {0}")]
    TestFailed(String),

    #[error("name already exists in this workspace")]
    NameTaken,

    #[error("access denied")]
    Forbidden,

    #[error("encryption error: {0}")]
    Vault(String),

    #[error("driver error: {0}")]
    Driver(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Database error, Invalid Database Type: {0}")]
    InvalidDbType(String),
}

impl IntoResponse for ConnectionError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            ConnectionError::NotFound => (StatusCode::NOT_FOUND, 404, self.to_string()),

            ConnectionError::NameTaken => (StatusCode::CONFLICT, 409, self.to_string()),

            ConnectionError::Forbidden => (StatusCode::FORBIDDEN, 403, self.to_string()),

            ConnectionError::UnsupportedDb(_) => (StatusCode::BAD_REQUEST, 400, self.to_string()),

            ConnectionError::TestFailed(_) => (StatusCode::BAD_GATEWAY, 502, self.to_string()),

            ConnectionError::Driver(_) => (StatusCode::BAD_GATEWAY, 502, self.to_string()),

            ConnectionError::Database(_) => (StatusCode::BAD_GATEWAY, 502, self.to_string()),

            ConnectionError::InvalidDbType(_) => (StatusCode::BAD_REQUEST, 400, self.to_string()),

            ConnectionError::Vault(_) | ConnectionError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                500,
                "internal server error".to_string(),
            ),
        };

        (status, ApiResponse::<()>::error((), message, code)).into_response()
    }
}
