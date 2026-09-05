use super::provider::{OAuthConfig, OAuthProviderClient, PkceChallenge};
use crate::features::auth::domains::{OAuthProfile, OAuthProvider, TokenResponse};
use crate::features::auth::auth_error::AuthError;
use async_trait::async_trait;
use serde::Deserialize;

pub struct GoogleProvider {
    config: OAuthConfig,
    http: reqwest::Client,
}

impl GoogleProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: String,
    name: String,
    picture: Option<String>,
    email_verified: Option<bool>,
}

#[async_trait]
impl OAuthProviderClient for GoogleProvider {
    fn provider(&self) -> OAuthProvider {
        OAuthProvider::Google
    }

    fn authorization_url(&self, state: &str, challenge: &PkceChallenge) -> String {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth\
             ?client_id={}\
             &redirect_uri={}\
             &response_type=code\
             &scope=openid%20email%20profile\
             &state={}\
             &code_challenge={}\
             &code_challenge_method=S256\
             &access_type=offline\
             &prompt=select_account",
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
        // 1. Exchange code for tokens
        let token_res: TokenResponse = self
            .http
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("client_secret", self.config.client_secret.as_str()),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("code_verifier", code_verifier),
            ])
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

        // 2. Fetch user info
        let user: GoogleUserInfo = self
            .http
            .get("https://www.googleapis.com/oauth2/v3/userinfo")
            .bearer_auth(&token_res.access_token)
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

        Ok(OAuthProfile {
            provider: OAuthProvider::Google,
            provider_user_id: user.sub,
            email: user.email,
            display_name: user.name,
            avatar_url: user.picture,
        })
    }
}
