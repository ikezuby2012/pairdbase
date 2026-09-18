use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use schemars::JsonSchema;

use crate::{
    abstractions::responses::api_response::ApiResponse,
    features::auth::{
        auth_error::{AuthApiError, AuthError},
        domains::{AuthTokens, OAuthProvider},
        oauth::provider::{generate_pkce, generate_state},
        use_cases::AuthUseCases,
    },
    state::AppState,
};

#[derive(Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OAuthCallbackParams {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OAuthRedirectResponse {
    pub authorization_url: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> Result<ApiResponse<AuthTokens>, AuthApiError> {
    let org_id = uuid::Uuid::new_v4();

    let tokens = state.auth
        .register(org_id, body.email, body.password, body.display_name)
        .await?;

    Ok(ApiResponse::created(
        tokens,
        "Account created successfully",
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<ApiResponse<AuthTokens>, AuthApiError> {
    let tokens = state.auth.login(&body.email, &body.password).await?;
    Ok(ApiResponse::success(tokens, "Login successful"))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<ApiResponse<AuthTokens>, AuthApiError> {
    let tokens = state.auth.refresh(&body.refresh_token).await?;
    Ok(ApiResponse::success(tokens, "Tokens refreshed successfully"))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<StatusCode, AuthApiError> {
    state.auth.logout(&body.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Step 1: Generate OAuth authorization URL
pub async fn oauth_redirect(
    State(state): State<Arc<AppState>>,
    Path(provider_slug): Path<String>,
) -> Result<ApiResponse<OAuthRedirectResponse>, AuthApiError> {
    let provider = state
        .oauth_providers
        .get(&provider_slug)
        .ok_or(AuthApiError::UnsupportedProvider)?;

    let state_token = generate_state();
    let pkce = generate_pkce();

    let url = provider.authorization_url(
        &state_token,
        &pkce,
    );

    // Persist PKCE state
    state
        .auth_repo
        .store_oauth_state(
            &state_token,
            &provider_slug,
            &pkce.code_verifier,
            None,
        )
        .await
        .map_err(|e| AuthApiError::Internal(e.to_string()))?;

    Ok(ApiResponse::success(
        OAuthRedirectResponse {
            authorization_url: url,
        },
        "OAuth authorization URL generated",
    ))
}

/// Step 2: Provider redirects back here with code + state
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Path(provider_slug): Path<String>,
    Query(params): Query<OAuthCallbackParams>,
) -> Result<ApiResponse<AuthTokens>, AuthApiError> {
    // 1. Consume and validate PKCE state
    let oauth_state = state
        .auth_repo
        .consume_oauth_state(&params.state)
        .await
        .map_err(|e| AuthApiError::Internal(e.to_string()))?
        .ok_or(AuthApiError::InvalidState)?;

    // Make sure the state belongs to this provider
    if oauth_state.provider != provider_slug {
        return Err(AuthApiError::InvalidState);
    }

    // 2. Get provider client
    let provider = state
        .oauth_providers
        .get(&provider_slug)
        .ok_or(AuthApiError::UnsupportedProvider)?;

    // 3. Exchange authorization code for profile
    let profile = provider
        .exchange_code(
            &params.code,
            &oauth_state.code_verifier,
        )
        .await
        .map_err(|e| AuthApiError::Internal(e.to_string()))?;

    // 4. Login or register
    let org_id = uuid::Uuid::new_v4(); // TODO: resolve properly

    let tokens = state
        .auth
        .oauth_login_or_register(org_id, profile)
        .await?;

    // 5. Return tokens to client
    Ok(ApiResponse::success(
        tokens,
        "OAuth authentication successful",
    ))
}
