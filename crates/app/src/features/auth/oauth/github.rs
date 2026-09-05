use super::provider::{OAuthConfig, OAuthProviderClient, PkceChallenge};
use crate::features::auth::domains::{OAuthProfile, OAuthProvider, TokenResponse};
use crate::features::auth::auth_error::AuthError;
use async_trait::async_trait;
use serde::Deserialize;

pub struct GitHubProvider {
    config: OAuthConfig,
    http: reqwest::Client,
}

impl GitHubProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id:         u64,
    login:      String,
    name:       Option<String>,
    avatar_url: Option<String>,
    email:      Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubEmail {
    email:    String,
    primary:  bool,
    verified: bool,
}

#[async_trait]
impl OAuthProviderClient for GitHubProvider {
    fn provider(&self) -> OAuthProvider {
        OAuthProvider::GitHub
    }

    fn authorization_url(&self, state: &str, challenge: &PkceChallenge) -> String {
        // GitHub supports PKCE but it's optional — we include it for consistency
        format!(
            "https://github.com/login/oauth/authorize\
             ?client_id={}\
             &redirect_uri={}\
             &scope=read:user%20user:email\
             &state={}\
             &code_challenge={}\
             &code_challenge_method=S256",
            self.config.client_id,
            urlencoding::encode(&self.config.redirect_uri),
            state,
            challenge.code_challenge,
        )
    }

    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthProfile, AuthError> {
        // 1. Exchange code for access token
        let token_res: TokenResponse = self
            .http
            .post("https://github.com/login/oauth/access_token")
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("code", code),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

        // 2. Fetch user profile
        let user: GitHubUser = self
            .http
            .get("https://api.github.com/user")
            .bearer_auth(&token_res.access_token)
            .header("User-Agent", "queryforge/1.0")
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

        // 3. GitHub may not expose email on profile — fetch from /user/emails
        let email = match user.email {
            Some(e) => e,
            None => {
                let emails: Vec<GitHubEmail> = self
                    .http
                    .get("https://api.github.com/user/emails")
                    .bearer_auth(&token_res.access_token)
                    .header("User-Agent", "queryforge/1.0")
                    .send()
                    .await
                    .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

                emails
                    .into_iter()
                    .find(|e| e.primary && e.verified)
                    .map(|e| e.email)
                    .ok_or_else(|| {
                        AuthError::OAuthFailed(
                            "no verified primary email on GitHub account".to_string(),
                        )
                    })?
            }
        };

        Ok(OAuthProfile {
            provider: OAuthProvider::GitHub,
            provider_user_id: user.id.to_string(),
            email,
            display_name: user.name.unwrap_or(user.login),
            avatar_url: user.avatar_url,
        })
    }
}
