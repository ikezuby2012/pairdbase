use super::provider::{OAuthConfig, OAuthProviderClient, PkceChallenge};
use crate::features::auth::domains::{OAuthProfile, OAuthProvider, TokenResponse};
use crate::features::auth::auth_error::AuthError;
use async_trait::async_trait;
use serde::Deserialize;

// X/Twitter uses OAuth 2.0 with PKCE (their v2 API)
pub struct TwitterProvider {
    config: OAuthConfig,
    http: reqwest::Client,
}

impl TwitterProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct TwitterUserData {
    id: String,
    name: String,
    username: String,
    profile_image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TwitterUserResponse {
    data: TwitterUserData,
}

#[async_trait]
impl OAuthProviderClient for TwitterProvider {
    fn provider(&self) -> OAuthProvider {
        OAuthProvider::Twitter
    }

    fn authorization_url(&self, state: &str, challenge: &PkceChallenge) -> String {
        format!(
            "https://twitter.com/i/oauth2/authorize\
             ?client_id={}\
             &redirect_uri={}\
             &response_type=code\
             &scope=tweet.read%20users.read%20offline.access\
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
        // Twitter v2 requires Basic auth on token endpoint
        let token_res: TokenResponse = self
            .http
            .post("https://api.twitter.com/2/oauth2/token")
            .basic_auth(&self.config.client_id, Some(&self.config.client_secret))
            .form(&[
                ("grant_type", "authorization_code"),
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

        let res: TwitterUserResponse = self
            .http
            .get("https://api.twitter.com/2/users/me?user.fields=profile_image_url")
            .bearer_auth(&token_res.access_token)
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(e.to_string()))?;

        let user = res.data;

        // Twitter OAuth 2.0 doesn't return email — generate a placeholder
        // Users can add/verify email after sign-up
        let email = format!("{}@twitter.queryforge.placeholder", user.username);

        Ok(OAuthProfile {
            provider: OAuthProvider::Twitter,
            provider_user_id: user.id,
            email,
            display_name: user.name,
            avatar_url: user.profile_image_url,
        })
    }
}
