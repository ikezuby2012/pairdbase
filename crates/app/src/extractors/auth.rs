use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use uuid::Uuid;
use schemars::JsonSchema;

use crate::state::AppState;
use std::sync::Arc;

#[derive(Debug, Clone, JsonSchema)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub org_id:  Uuid,
    pub role:    String,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
     type Rejection = AuthRejection;

     async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract Bearer token from Authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| AuthRejection::MissingToken)?;

        // 2. Validate JWT and extract claims
        let claims = state
            .token_service
            .verify_access_token(bearer.token())
            .map_err(|_| AuthRejection::InvalidToken)?;

        // 3. Parse UUIDs from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AuthRejection::InvalidToken)?;

        let org_id = Uuid::parse_str(&claims.org_id)
            .map_err(|_| AuthRejection::InvalidToken)?;

        Ok(AuthUser {
            user_id,
            org_id,
            role: claims.role,
        })
    }
}

#[derive(Debug)]
pub enum AuthRejection {
    MissingToken,
    InvalidToken,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthRejection::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "missing authorization header",
            ),
            AuthRejection::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "invalid or expired token",
            ),
        };

        (
            status,
            Json(serde_json::json!({ "error": message })),
        )
        .into_response()
    }
}

impl aide::OperationInput for AuthUser {}