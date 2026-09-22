use axum::{
    http::StatusCode
};
use schemars::JsonSchema;

#[derive(Debug, thiserror::Error, JsonSchema)]
pub enum OrgError {
    #[error("organization not found")]
    NotFound,

    #[error("slug already taken")]
    SlugTaken,

    #[error("user is already a member")]
    AlreadyMember,

    #[error("invitation not found or expired")]
    InvitationNotFound,

    #[error("invitation already accepted")]
    InvitationAlreadyAccepted,

    #[error("cannot remove the organization owner")]
    CannotRemoveOwner,

    #[error("access denied")]
    Forbidden,

    #[error("plan limit reached: {0}")]
    PlanLimitReached(String),

    #[error("invalid plan: {0}")]
    InvalidPlan(String),

    #[error("invalid role: {0}")]
    InvalidRole(String),

    #[error("internal error: {0}")]
    Internal(String),
}

shared::impl_api_error! {
    OrgError {
        OrgError::NotFound => (
            StatusCode::NOT_FOUND,
            "ORG_NOT_FOUND",
            "organization not found".to_string()
        ),

        OrgError::SlugTaken => (
            StatusCode::CONFLICT,
            "ORG_SLUG_TAKEN",
            "organization slug already taken".to_string()
        ),

        OrgError::AlreadyMember => (
            StatusCode::CONFLICT,
            "ORG_ALREADY_MEMBER",
            "user is already a member".to_string()
        ),

        OrgError::InvitationNotFound => (
            StatusCode::NOT_FOUND,
            "ORG_INVITATION_NOT_FOUND",
            "invitation not found or expired".to_string()
        ),

        OrgError::InvitationAlreadyAccepted => (
            StatusCode::CONFLICT,
            "ORG_INVITATION_ALREADY_ACCEPTED",
            "invitation already accepted".to_string()
        ),

        OrgError::CannotRemoveOwner => (
            StatusCode::BAD_REQUEST,
            "ORG_CANNOT_REMOVE_OWNER",
            "cannot remove the organization owner".to_string()
        ),

        OrgError::Forbidden => (
            StatusCode::FORBIDDEN,
            "ORG_FORBIDDEN",
            "access denied".to_string()
        ),

        OrgError::PlanLimitReached(message) => (
            StatusCode::FORBIDDEN,
            "ORG_PLAN_LIMIT_REACHED",
            format!("plan limit reached: {message}")
        ),

        OrgError::InvalidPlan(message) => (
            StatusCode::BAD_REQUEST,
            "ORG_INVALID_PLAN",
            format!("invalid plan: {message}")
        ),

        OrgError::InvalidRole(message) => (
            StatusCode::BAD_REQUEST,
            "ORG_INVALID_ROLE",
            format!("invalid role: {message}")
        ),

        OrgError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "internal server error".to_string()
        ),
    }
}
