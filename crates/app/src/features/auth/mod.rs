use aide::{
    axum::{
        routing::{delete_with, get_with, post_with, put_with},
        ApiRouter,
    },
    transform::TransformOperation,
};

use crate::abstractions::responses::{ApiErrorResponse, ApiResponseSchema};
use axum::{
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

pub mod auth_error;
pub mod domains;
pub mod handler;
pub mod oauth;
pub mod repository;
pub mod tokens;
pub mod use_cases;

use crate::state::AppState;
use crate::{db::DbPool, features::auth::repository::AuthRepo};
use auth_error::AuthApiError;
use domains::AuthTokensResponse;
use repository::PgAuthRepo;
use tokens::TokenService;
use use_cases::AuthUseCases;

pub fn mount(
    pool: DbPool,
    tokens: Arc<TokenService>,
) -> (ApiRouter<Arc<AppState>>, Arc<AuthUseCases>) {
    let repo = Arc::new(PgAuthRepo::new(pool.clone()));
    let uc = Arc::new(AuthUseCases::new(repo, tokens));

    let router = ApiRouter::new()
        .api_route("/register", post_with(handler::register, register_docs))
        .api_route("/login", post_with(handler::login, login_docs))
        .api_route("/refresh", post_with(handler::refresh, refresh_docs))
        .api_route("/logout", post_with(handler::logout, logout_docs))
        .api_route(
            "/oauth/{provider}",
            get_with(handler::oauth_redirect, oauth_redirect_docs),
        )
        .route(
            "/oauth/{provider}/callback",
            get(handler::oauth_callback),
        );

    (router, uc)
}

fn register_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Register a new user")
        .tag("auth")
        .response::<200, Json<ApiResponseSchema<AuthTokensResponse>>>()
        .response_with::<409, Json<ApiErrorResponse>, _>(|r| {
            r.description("Email already registered")
        })
}

fn login_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Login with email and password")
        .tag("auth")
        .response::<200, Json<ApiResponseSchema<AuthTokensResponse>>>()
        .response_with::<401, Json<ApiErrorResponse>, _>(|r| r.description("Invalid credentials"))
}

fn refresh_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Refresh access token")
        .description(
            "Exchange a refresh token for a new access token. Old refresh token is revoked.",
        )
        .tag("auth")
        .response::<200, Json<ApiResponseSchema<AuthTokensResponse>>>()
        .response_with::<401, Json<ApiErrorResponse>, _>(|r| {
            r.description("Invalid or expired refresh token")
        })
}

fn logout_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Logout")
        .description("Revokes the provided refresh token.")
        .tag("auth")
        .response::<204, ()>()
        .security_requirement("bearer_auth")
}

fn oauth_redirect_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Start OAuth flow")
        .description(
            "Redirects to the OAuth provider. Supported: google, github, twitter, facebook",
        )
        .tag("auth")
        .response_with::<302, (), _>(|r| r.description("Redirect to provider"))
        .response_with::<400, Json<ApiErrorResponse>, _>(|r| r.description("Unsupported provider"))
}

fn oauth_callback_docs(op: TransformOperation) -> TransformOperation {
    op.summary("OAuth callback")
        .description(
            "Completes the OAuth authorization flow by validating the \
             OAuth state, exchanging the authorization code, and issuing \
             authentication tokens.",
        )
        .tag("auth")
}
