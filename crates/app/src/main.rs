use aide::openapi::{Info, OpenApi};
use axum::{Extension, Json, Router};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use ::redis::aio::ConnectionManager;

mod abstractions;
mod config;
mod db;
mod docs;
mod extractors;
mod features;
mod redis;
mod routes;
mod state;

use crate::config::AppConfig;
use crate::db::create_pool;
use crate::features::auth::mount as auth_mount;
use crate::features::auth::oauth;
use crate::features::auth::repository::{AuthRepo, PgAuthRepo};
use crate::features::auth::tokens::TokenService;
use crate::features::auth::use_cases::AuthUseCases;
use crate::features::connections::pool::DriverRegistry;
use crate::features::connections::pool::SchemaCache;
use crate::features::connections::repository::PgConnectionRepo;
use crate::features::connections::use_cases::ConnectionUseCases;
use crate::features::connections::vault::Vault;
use crate::features::query::repository::PgQueryRepo;
use crate::features::query::use_case::QueryUseCases;
use crate::features::query::{pool::ExecutionPoolRegistry, session::SessionRegistry};

use crate::redis::create_client;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    tracing::info!("starting PairDBase");

    let config = AppConfig::load()?;
    let db = create_pool(
        &config.database.url,
        config.database.max_conns,
        config.database.min_conns,
    )
    .await?;

    // Build OpenAPI document
    let mut api = OpenApi {
        info: Info {
            title: "PairDBase API".to_string(),
            ..Info::default()
        },
        ..OpenApi::default()
    };

    let jwt_secret = config.auth.jwt_secret.clone();
    let vault_key = config.vault.key.clone();

    let redis = create_client(&config.redis.url)?;
    let redis_conn_manager = ConnectionManager::new(redis.clone()).await?;
    let auth_repo: Arc<dyn AuthRepo> = Arc::new(PgAuthRepo::new(db.clone()));
    let registry = Arc::new(DriverRegistry::new());
    let schema_cache = Arc::new(SchemaCache::new(redis_conn_manager));

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

    let vault = Arc::new(Vault::new(&vault_key)?);
    tracing::info!("vault initialised");

    let (auth_router, auth) = auth_mount(db.clone(), Arc::clone(&token_service));

    let conn_repo = Arc::new(PgConnectionRepo::new(db.clone(), Arc::clone(&vault)));
    let query_repo = Arc::new(PgQueryRepo::new(db.clone()));
    let sessions = Arc::new(SessionRegistry::new());
    let exec_pools = Arc::new(ExecutionPoolRegistry::new());

    let connections = Arc::new(ConnectionUseCases::new(
        conn_repo.clone(),
        vault,
        registry,
        schema_cache,
    ));

    let query_case = Arc::new(QueryUseCases::new(
        query_repo,
        sessions,
        exec_pools,
        connections.clone(),
    ));

    let state = Arc::new(AppState {
        db,
        redis,
        config: config.clone(),
        auth,
        oauth_providers,
        auth_repo,
        token_service,
        connections,
        query: query_case,
    });

    let api_router = routes::router(auth_router).finish_api(&mut api);

    let app = Router::new()
        .nest("/api/v1", api_router.into())
        .route("/api-docs/openapi.json", axum::routing::get(serve_openapi))
        .route("/docs", axum::routing::get(serve_scalar))
        // Attach OpenAPI doc to app state
        .layer(Extension(Arc::new(api)))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive()) // tighten in production
        .with_state(state);

    let addr = format!("{}:{}", config.server.host, config.server.api_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("API server listening on {addr}");

    axum::serve(listener, app).await?;
    Ok(())
}

// ── Serve OpenAPI JSON ────────────────────────────────────────────────────────

async fn serve_openapi(Extension(openapi): Extension<Arc<OpenApi>>) -> Json<OpenApi> {
    Json((*openapi).clone())
}

// ── Serve Scalar UI ───────────────────────────────────────────────────────────

async fn serve_scalar() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>QueryForge API</title>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
    </head>
    <body>
        <script
            id="api-reference"
            data-url="/api-docs/openapi.json"
            data-configuration='{"theme":"purple"}'
        ></script>
        <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
    </body>
    </html>
    "#,
    )
}
