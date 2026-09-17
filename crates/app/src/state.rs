use std::sync::Arc;

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::features::auth::oauth::ProviderRegistry;
use crate::features::auth::repository::AuthRepo;
use crate::features::auth::tokens::TokenService;
use crate::features::auth::use_cases::AuthUseCases;
use crate::features::connections::use_cases::ConnectionUseCases;
use crate::features::query::use_case::QueryUseCases;
use crate::redis::RedisClient;

pub struct AppState {
    pub db: DbPool,
    pub redis: RedisClient,
    pub config: AppConfig,

    pub auth: Arc<AuthUseCases>,
    pub oauth_providers: ProviderRegistry,
    pub auth_repo: Arc<dyn AuthRepo>,
    pub token_service: Arc<TokenService>,

    pub connections: Arc<ConnectionUseCases>,
    pub query: Arc<QueryUseCases>,
}

pub type SharedState = Arc<AppState>;
