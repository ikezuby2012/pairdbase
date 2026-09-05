pub mod provider;
pub mod github;
pub mod google;
pub mod twitter;

use std::{collections::HashMap, sync::Arc};
use provider::OAuthProviderClient;
use crate::features::auth::domains::OAuthProvider;

pub type ProviderRegistry = HashMap<String, Arc<dyn OAuthProviderClient>>;
pub fn build_registry(base_redirect_uri: &str) -> ProviderRegistry {
    use provider::OAuthConfig;
    let mut map: ProviderRegistry = HashMap::new();

    macro_rules! register {
        ($provider:ident, $env_prefix:literal, $slug:literal) => {
            let client_id = std::env::var(concat!($env_prefix, "_CLIENT_ID")).ok();
            let client_secret = std::env::var(concat!($env_prefix, "_CLIENT_SECRET")).ok();

            if let (Some(id), Some(secret)) = (client_id, client_secret) {
                let config = OAuthConfig {
                    client_id:     id,
                    client_secret: secret,
                    redirect_uri:  format!("{}/auth/callback/{}", base_redirect_uri, $slug),
                };
                map.insert($slug.to_string(), Arc::new($provider::new(config)));
            }
        };
    }

    use google::GoogleProvider;
    use github::GitHubProvider;
    use twitter::TwitterProvider;

    register!(GoogleProvider,   "GOOGLE",   "google");
    register!(GitHubProvider,   "GITHUB",   "github");
    register!(TwitterProvider,  "TWITTER",  "twitter");

    map
}