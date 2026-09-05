use axum::Router;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod abstractions;
mod config;
mod db;
mod features;
mod redis;
mod routes;
mod state;

use crate::config::AppConfig;
use crate::db::create_pool;
use crate::features::auth::repository::{AuthRepo, PgAuthRepo};
use crate::features::auth::tokens::TokenService;
use crate::features::auth::use_cases::AuthUseCases;
use crate::features::auth::oauth;
use crate::redis::create_client;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = AppConfig::load()?;
    let db = create_pool(
        &config.database.url,
        config.database.max_conns,
        config.database.min_conns,
    )
    .await?;

    let jwt_secret = config.auth.jwt_secret.clone();

    let redis = create_client(&config.redis.url)?;
    let auth_repo: Arc<dyn AuthRepo> = Arc::new(PgAuthRepo::new(db.clone()));

    // ── Auth setup ────────────────────────────────────────────────────────────
    let token_service = Arc::new(TokenService::new(
        &jwt_secret,
        15 * 60,           // access token:  15 minutes
        30 * 24 * 60 * 60, // refresh token: 30 days
    ));

    let auth = Arc::new(AuthUseCases::new(
        Arc::clone(&auth_repo),
        Arc::clone(&token_service),
    ));

    let oauth_providers = oauth::build_registry(&config.base_url);

    tracing::info!(
        providers = ?oauth_providers.keys().collect::<Vec<_>>(),
        "OAuth providers registered"
    );

    
    let state = Arc::new(AppState {
        db,
        redis,
        config: config.clone(),
        auth,
        oauth_providers,
    });

    let app = Router::new()
        .nest("/api/v1", routes::router())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive()) // tighten in production
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.api_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
