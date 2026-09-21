use aide::operation::OperationOutput;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use schemars::JsonSchema;

#[derive(Debug, thiserror::Error, JsonSchema)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,

    #[error("email already registered")]
    EmailTaken,

    #[error("account not verified")]
    NotVerified,

    #[error("token expired or invalid")]
    InvalidToken,

    #[error("unsupported OAuth provider: {0}")]
    UnsupportedProvider(String),

    #[error("OAuth error: {0}")]
    OAuthFailed(String),

    #[error("user not found")]
    NotFound,

    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthApiError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("email already registered")]
    EmailTaken,
    #[error("token invalid or expired")]
    InvalidToken,
    #[error("unsupported provider")]
    UnsupportedProvider,
    #[error("invalid OAuth state — possible CSRF")]
    InvalidState,
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<AuthError> for AuthApiError {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::InvalidCredentials => AuthApiError::InvalidCredentials,
            AuthError::EmailTaken         => AuthApiError::EmailTaken,
            AuthError::InvalidToken       => AuthApiError::InvalidToken,
            AuthError::UnsupportedProvider(p) => AuthApiError::Internal(p),
            other => AuthApiError::Internal(other.to_string()),
        }
    }
}

shared::impl_api_error! {
    AuthApiError {
        AuthApiError::InvalidCredentials => (
            StatusCode::UNAUTHORIZED,
            "AUTH_INVALID_CREDENTIALS",
            "invalid credentials".to_string()
        ),

        AuthApiError::EmailTaken => (
            StatusCode::CONFLICT,
            "AUTH_EMAIL_TAKEN",
            "email is already registered".to_string()
        ),

        AuthApiError::InvalidToken => (
            StatusCode::UNAUTHORIZED,
            "AUTH_INVALID_TOKEN",
            "invalid token".to_string()
        ),

        AuthApiError::UnsupportedProvider => (
            StatusCode::BAD_REQUEST,
            "AUTH_UNSUPPORTED_PROVIDER",
            "unsupported authentication provider".to_string()
        ),

        AuthApiError::InvalidState => (
            StatusCode::BAD_REQUEST,
            "AUTH_INVALID_STATE",
            "invalid authentication state".to_string()
        ),

        AuthApiError::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "internal server error".to_string()
        ),
    }
}