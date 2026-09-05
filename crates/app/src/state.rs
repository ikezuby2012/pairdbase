use std::sync::Arc;

use crate::config::AppConfig;
use crate::db::DbPool;
use crate::features::auth::use_cases::AuthUseCases;
use crate::redis::RedisClient;
use crate::features::auth::oauth::ProviderRegistry;

pub struct AppState {
    pub db:     DbPool,
    pub redis:  RedisClient,
    pub config: AppConfig,

    pub auth: Arc<AuthUseCases>,
    pub oauth_providers: ProviderRegistry,
}

pub type SharedState = Arc<AppState>;