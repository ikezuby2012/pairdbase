use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub redis:    RedisConfig,
    pub auth:     AuthConfig,
    pub server:   ServerConfig,
    pub base_url: String,
    pub google: OAuthProviderConfig,
    pub github: OAuthProviderConfig,
    pub twitter: OAuthProviderConfig,
    pub facebook: OAuthProviderConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_conns: u32,
    pub min_conns: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub jwt_secret:          String,
    pub jwt_expiry_secs:     u64,
    pub refresh_expiry_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host:     String,
    pub api_port: u16,
    pub ws_port:  u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AiConfig {
    pub openai_api_key:    Option<String>,
    pub anthropic_api_key: Option<String>,
    pub default_model:     String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config_path = concat!(env!("CARGO_MANIFEST_DIR"), "/config/default");

        dotenvy::dotenv().ok();
        let cfg = config::Config::builder()
            .add_source(config::File::with_name(config_path))
            .add_source(config::Environment::with_prefix("PAIRDBASE").separator("__"))
            .build()?;
        Ok(cfg.try_deserialize()?)
    }
}
