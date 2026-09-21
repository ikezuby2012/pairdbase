use aide::operation::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

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

shared::impl_api_error! {
    QueryError {
        QueryError::NotConnected => (
            StatusCode::BAD_REQUEST,
            "QUERY_NOT_CONNECTED",
            "not connected".to_string()
        ),

        QueryError::SessionExpired => (
            StatusCode::UNAUTHORIZED,
            "QUERY_SESSION_EXPIRED",
            "query session expired".to_string()
        ),

        QueryError::NoSession => (
            StatusCode::UNAUTHORIZED,
            "QUERY_NO_SESSION",
            "no active query session".to_string()
        ),

        QueryError::Rejected(message) => (
            StatusCode::FORBIDDEN,
            "QUERY_REJECTED",
            format!("query rejected: {message}")
        ),

        QueryError::ExecutionFailed(message) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "QUERY_EXECUTION_FAILED",
            format!("query execution failed: {message}")
        ),

        QueryError::Timeout(message) => (
            StatusCode::REQUEST_TIMEOUT,
            "QUERY_TIMEOUT",
            format!("query timed out: {message}")
        ),

        QueryError::PermissionError(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "QUERY_PERMISSION_ERROR",
            "permission lookup failed".to_string()
        ),

        QueryError::ConnectionNotFound => (
            StatusCode::NOT_FOUND,
            "CONNECTION_NOT_FOUND",
            "connection not found".to_string()
        ),

        QueryError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "internal error".to_string()
        ),
    }
}
