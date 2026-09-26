//! Discord OAuth2 with PKCE.
//!
//! Scopes: `identify email`
//! Discord returns a user object on /users/@me — no separate email endpoint needed.
//! Avatar URL format: https://cdn.discordapp.com/avatars/{id}/{avatar}.png

use async_trait::async_trait;
use serde::Deserialize;

// use super::provider::{OAuthProvider, OAuthTokens, OAuthUser, PkceParams};
use super::provider::{OAuthConfig, OAuthProviderClient, PkceChallenge};
use crate::features::auth::auth_error::AuthError;
use crate::features::auth::domains::{OAuthProfile, OAuthProvider, TokenResponse};

pub struct DiscordProvider {
    // client_id:     String,
    // client_secret: String,
    // redirect_uri:  String,
    config: OAuthConfig,
    http: reqwest::Client,
}

impl DiscordProvider {
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
        }
    }
}

// ── Discord API response shapes ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct DiscordTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    discriminator: String, // "0" on new usernames, "1234" on legacy
    global_name: Option<String>,
    email: Option<String>,
    verified: Option<bool>,
    avatar: Option<String>,
    bot: Option<bool>,
}

impl DiscordUser {
    /// Canonical display name: global_name if set, else username#discriminator
    fn display_name(&self) -> String {
        if let Some(name) = &self.global_name {
            if !name.is_empty() {
                return name.clone();
            }
        }
        // Legacy discriminator display
        if self.discriminator != "0" {
            format!("{}#{}", self.username, self.discriminator)
        } else {
            self.username.clone()
        }
    }

    /// CDN avatar URL, falling back to the default Discord avatar
    fn avatar_url(&self) -> Option<String> {
        match &self.avatar {
            Some(hash) if !hash.is_empty() => Some(format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png?size=256",
                self.id, hash
            )),
            // Default avatar — index derived from discriminator or user id
            _ => {
                let index = if self.discriminator == "0" {
                    // New username system: (user_id >> 22) % 6
                    self.id.parse::<u64>().map(|id| (id >> 22) % 6).unwrap_or(0)
                } else {
                    self.discriminator.parse::<u64>().unwrap_or(0) % 5
                };
                Some(format!(
                    "https://cdn.discordapp.com/embed/avatars/{}.png",
                    index
                ))
            }
        }
    }
}

// ── OAuthProvider impl ────────────────────────────────────────────────────────
//OAuthProviderClient
#[async_trait]
impl OAuthProviderClient for DiscordProvider {
    fn provider(&self) -> OAuthProvider {
        OAuthProvider::Discord
    }

    fn authorization_url(&self, state: &str, pkce: &PkceChallenge) -> String {
        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("redirect_uri", self.config.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "identify email"),
            ("state", state),
            ("code_challenge", pkce.code_challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("prompt", "none"), // skip consent if already authorized
        ];

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        format!("https://discord.com/oauth2/authorize?{}", query)
    }

    /// Exchange code → tokens → user profile → OAuthProfile
    async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<OAuthProfile, AuthError> {
        // ── Step 1: exchange code for tokens ──────────────────────────────────
        let token_params: &[(&str, &str)] = &[
            ("client_id", &self.config.client_id),
            ("client_secret", &self.config.client_secret),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &self.config.redirect_uri),
            ("code_verifier", code_verifier),
        ];

        let token_resp = self
            .http
            .post("https://discord.com/api/oauth2/token")
            .form(token_params)
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(format!("Discord token request: {}", e)))?;

        if !token_resp.status().is_success() {
            let body = token_resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthFailed(format!(
                "Discord token exchange failed: {}",
                body
            )));
        }

        let tokens: DiscordTokenResponse = token_resp
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(format!("Discord token parse: {}", e)))?;

        // ── Step 2: fetch user profile ────────────────────────────────────────
        let user_resp = self
            .http
            .get("https://discord.com/api/users/@me")
            .bearer_auth(&tokens.access_token)
            .send()
            .await
            .map_err(|e| AuthError::OAuthFailed(format!("Discord user request: {}", e)))?;

        if !user_resp.status().is_success() {
            let body = user_resp.text().await.unwrap_or_default();
            return Err(AuthError::OAuthFailed(format!(
                "Discord user fetch failed: {}",
                body
            )));
        }

        let user: DiscordUser = user_resp
            .json()
            .await
            .map_err(|e| AuthError::OAuthFailed(format!("Discord user parse: {}", e)))?;

        // ── Step 3: validate ──────────────────────────────────────────────────
        if user.bot == Some(true) {
            return Err(AuthError::OAuthFailed(
                "Bot accounts cannot sign in.".into(),
            ));
        }

        let email = user.email.clone().ok_or_else(|| {
            AuthError::OAuthFailed(
                "Discord account has no email address. \
                 Ensure your Discord account has a verified email and \
                 the 'email' scope is granted."
                    .into(),
            )
        })?;

        if user.verified != Some(true) {
            return Err(AuthError::OAuthFailed(
                "Discord email is not verified. \
                 Please verify your Discord email address first."
                    .into(),
            ));
        }

        // ── Step 4: return normalised profile ─────────────────────────────────
        Ok(OAuthProfile {
            provider: OAuthProvider::Discord,
            provider_user_id: user.id.clone(),
            email,
            display_name: user.display_name(),
            avatar_url: user.avatar_url(),
            // raw: serde_json::json!({
            //     "id":            user.id,
            //     "username":      user.username,
            //     "discriminator": user.discriminator,
            //     "global_name":   user.global_name,
            //     "verified":      user.verified,
            //     "scope":         tokens.scope,
            // }),
        })
    }
}
