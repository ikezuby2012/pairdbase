use crate::abstractions::ApiResponse;
use aide::operation::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use schemars::JsonSchema;

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error, JsonSchema)]
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

shared::impl_api_error! {
    ConnectionError {
        ConnectionError::NotFound => (
            StatusCode::NOT_FOUND,
            "CONNECTION_NOT_FOUND",
            "connection not found".to_string()
        ),

        ConnectionError::NameTaken => (
            StatusCode::CONFLICT,
            "CONNECTION_NAME_TAKEN",
            "connection name already exists".to_string()
        ),

        ConnectionError::Forbidden => (
            StatusCode::FORBIDDEN,
            "CONNECTION_FORBIDDEN",
            "access denied".to_string()
        ),

        ConnectionError::UnsupportedDb(message) => (
            StatusCode::BAD_REQUEST,
            "CONNECTION_UNSUPPORTED_DB",
            format!("unsupported database: {message}")
        ),

        ConnectionError::TestFailed(message) => (
            StatusCode::BAD_GATEWAY,
            "CONNECTION_TEST_FAILED",
            format!("connection test failed: {message}")
        ),

        ConnectionError::Driver(message) => (
            StatusCode::BAD_GATEWAY,
            "CONNECTION_DRIVER_ERROR",
            format!("driver error: {message}")
        ),

        ConnectionError::Database(message) => (
            StatusCode::BAD_GATEWAY,
            "CONNECTION_DATABASE_ERROR",
            format!("database error: {message}")
        ),

        ConnectionError::InvalidDbType(message) => (
            StatusCode::BAD_REQUEST,
            "CONNECTION_INVALID_DB_TYPE",
            format!("invalid database type: {message}")
        ),

        ConnectionError::Vault(_) | ConnectionError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "internal server error".to_string()
        ),
    }
}