pub mod auth_error;
pub mod domains;
pub mod handler;
pub mod oauth;
pub mod repository;
pub mod tokens;
pub mod use_cases;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::{db::DbPool, features::auth::repository::AuthRepo};
use repository::PgAuthRepo;
use tokens::TokenService;
use use_cases::AuthUseCases;

pub fn mount(
    pool: DbPool,
    tokens: Arc<TokenService>,
) -> (Router<Arc<AppState>>, Arc<AuthUseCases>) {
    let repo = Arc::new(PgAuthRepo::new(pool.clone()));
    let uc = Arc::new(AuthUseCases::new(repo, tokens));

    let router = Router::new()
        .route("/register", post(handler::register))
        .route("/login", post(handler::login))
        .route("/refresh", post(handler::refresh))
        .route("/logout", post(handler::logout))
        .route("/oauth/{provider}", get(handler::oauth_redirect))
        .route("/oauth/{provider}/callback", get(handler::oauth_callback));

    (router, uc)
}
