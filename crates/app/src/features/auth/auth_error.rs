use thiserror::Error;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

#[derive(Debug, thiserror::Error)]
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

impl IntoResponse for AuthApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AuthApiError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthApiError::EmailTaken         => (StatusCode::CONFLICT, self.to_string()),
            AuthApiError::InvalidToken       => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthApiError::UnsupportedProvider => (StatusCode::BAD_REQUEST, self.to_string()),
            AuthApiError::InvalidState        => (StatusCode::BAD_REQUEST, self.to_string()),
            AuthApiError::Internal(_)         => (StatusCode::INTERNAL_SERVER_ERROR, "internal server error".to_string()),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}