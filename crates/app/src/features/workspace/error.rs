use aide::operation::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace not found")]
    NotFound,

    #[error("workspace name already exists in this organization")]
    NameTaken,

    #[error("access denied")]
    Forbidden,

    #[error("user not found: {0}")]
    UserNotFound(String),

    #[error("user is already a member")]
    AlreadyMember,

    #[error("cannot remove the workspace owner")]
    CannotRemoveOwner,

    #[error("invalid role: {0}")]
    InvalidRole(String),

    #[error("internal error: {0}")]
    Internal(String),
}

shared::impl_api_error! {
    WorkspaceError {
        WorkspaceError::NotFound => (
            StatusCode::NOT_FOUND,
            "WORKSPACE_NOT_FOUND",
            "Workspace not found".to_string()
        ),

        WorkspaceError::NameTaken => (
            StatusCode::CONFLICT,
            "WORKSPACE_NAME_TAKEN",
            "Workspace name already exists in this organization".to_string()
        ),

        WorkspaceError::Forbidden => (
            StatusCode::FORBIDDEN,
            "WORKSPACE_FORBIDDEN",
            "Access denied".to_string()
        ),

        WorkspaceError::UserNotFound(message) => (
            StatusCode::NOT_FOUND,
            "WORKSPACE_USER_NOT_FOUND",
            format!("user not found: {message}")
        ),

        WorkspaceError::AlreadyMember => (
            StatusCode::CONFLICT,
            "WORKSPACE_ALREADY_MEMBER",
            "User is already a member".to_string()
        ),

        WorkspaceError::CannotRemoveOwner => (
            StatusCode::BAD_REQUEST,
            "WORKSPACE_CANNOT_REMOVE_OWNER",
            "Cannot remove the workspace owner".to_string()
        ),

        WorkspaceError::InvalidRole(message) => (
            StatusCode::BAD_REQUEST,
            "WORKSPACE_INVALID_ROLE",
            format!("invalid role: {message}")
        ),

        WorkspaceError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "An internal error occurred".to_string()
        ),
    }
}
