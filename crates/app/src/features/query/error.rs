use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use aide::operation::OperationOutput;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("not connected — call POST /query/connect first")]
    NotConnected,

    #[error("session expired — please reconnect")]
    SessionExpired,

    #[error("no active session for this connection — call /connect first")]
    NoSession,

    #[error("query rejected: {0}")]
    Rejected(String),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("query timed out after {0}s")]
    Timeout(u64),

    #[error("permission lookup failed: {0}")]
    PermissionError(String),

    #[error("connection not found")]
    ConnectionNotFound,

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for QueryError {
    fn into_response(self) -> Response {
        let (status, message, code) = match &self {
            QueryError::NotConnected => (
                StatusCode::BAD_REQUEST,
                self.to_string(),
                "QUERY_NOT_CONNECTED",
            ),

            QueryError::SessionExpired => (
                StatusCode::UNAUTHORIZED,
                self.to_string(),
                "QUERY_SESSION_EXPIRED",
            ),

            QueryError::NoSession => (
                StatusCode::UNAUTHORIZED,
                self.to_string(),
                "QUERY_NO_SESSION",
            ),

            QueryError::Rejected(_) => (StatusCode::FORBIDDEN, self.to_string(), "QUERY_REJECTED"),

            QueryError::ExecutionFailed(_) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                self.to_string(),
                "QUERY_EXECUTION_FAILED",
            ),

            QueryError::Timeout(_) => (
                StatusCode::REQUEST_TIMEOUT,
                self.to_string(),
                "QUERY_TIMEOUT",
            ),

            QueryError::PermissionError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "permission lookup failed".to_string(),
                "QUERY_PERMISSION_ERROR",
            ),

            QueryError::ConnectionNotFound => (
                StatusCode::NOT_FOUND,
                self.to_string(),
                "CONNECTION_NOT_FOUND",
            ),

            QueryError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
                "INTERNAL_ERROR",
            ),
        };

        (
            status,
            Json(json!({
                "status": "error",
                "message": message,
                "code": code,
                "data": null
            })),
        )
            .into_response()
    }
}

impl OperationOutput for QueryError {
    type Inner = Self;
}
