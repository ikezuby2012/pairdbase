use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;
// use rand_core::{OsRng, RngCore};

use crate::features::auth::{
    auth_error::AuthError,
    domains::{AuthTokens, NewUser, OAuthProfile, User},
    repository::AuthRepo,
    tokens::TokenService,
};

pub struct AuthUseCases {
    repo: Arc<dyn AuthRepo>,
    tokens: Arc<TokenService>,
}

impl AuthUseCases {
    pub fn new(repo: Arc<dyn AuthRepo>, tokens: Arc<TokenService>) -> Self {
        Self { repo, tokens }
    }

    pub async fn register(
        &self,
        org_id: Uuid,
        email: String,
        password: String,
        display_name: String,
    ) -> Result<AuthTokens, AuthError> {
        if self.repo.find_user_by_email(&email).await?.is_some() {
            return Err(AuthError::EmailTaken);
        }

        let hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|e| AuthError::Internal(e.to_string()))?
            .to_string();

        let user = self
            .repo
            .create_user(NewUser {
                organization_id: org_id,
                email,
                display_name,
                avatar_url: None,
                password_hash: Some(hash),
                is_verified: false, // send verification email separately
            })
            .await?;

        self.issue_tokens(&user).await
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AuthTokens, AuthError> {
        let user = self
            .repo
            .find_user_by_email(email)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        // Verify password
        let hash = user
            .password_hash
            .as_deref()
            .ok_or(AuthError::InvalidCredentials)?;

        let parsed = PasswordHash::new(hash).map_err(|_| AuthError::Internal("bad hash".into()))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| AuthError::InvalidCredentials)?;

        self.repo.update_last_login(user.id.0).await?;
        self.issue_tokens(&user).await
    }

    pub async fn oauth_login_or_register(
        &self,
        org_id: Uuid,
        profile: OAuthProfile,
    ) -> Result<AuthTokens, AuthError> {
        if let Some(account) = self
            .repo
            .find_oauth_account(&profile.provider, &profile.provider_user_id)
            .await?
        {
            // Existing OAuth user — load their user record and issue tokens
            let user = self
                .repo
                .find_user_by_id(account.user_id.0)
                .await?
                .ok_or(AuthError::NotFound)?;

            self.repo.update_last_login(user.id.0).await?;
            return self.issue_tokens(&user).await;
        }

        let user = match self.repo.find_user_by_email(&profile.email).await? {
            Some(existing) => {
                // Link this OAuth provider to the existing account
                self.repo
                    .create_oauth_account(
                        existing.id.0,
                        &profile.provider,
                        &profile.provider_user_id,
                    )
                    .await?;
                existing
            }
            None => {
                // Brand new user — create account + link OAuth
                let new_user = self
                    .repo
                    .create_user(NewUser {
                        organization_id: org_id,
                        email: profile.email,
                        display_name: profile.display_name,
                        avatar_url: profile.avatar_url,
                        password_hash: None, // OAuth only
                        is_verified: true,   // OAuth emails are pre-verified
                    })
                    .await?;

                self.repo
                    .create_oauth_account(
                        new_user.id.0,
                        &profile.provider,
                        &profile.provider_user_id,
                    )
                    .await?;

                new_user
            }
        };

        self.repo.update_last_login(user.id.0).await?;
        self.issue_tokens(&user).await
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthTokens, AuthError> {
        let hash = TokenService::hash_token(refresh_token);

        let user_id = self
            .repo
            .find_refresh_token(&hash)
            .await?
            .ok_or(AuthError::InvalidToken)?;

        // Rotate — revoke old, issue new (prevents token reuse)
        self.repo.revoke_refresh_token(&hash).await?;

        let user = self
            .repo
            .find_user_by_id(user_id)
            .await?
            .ok_or(AuthError::NotFound)?;

        self.issue_tokens(&user).await
    }

    pub async fn logout(&self, refresh_token: &str) -> Result<(), AuthError> {
        let hash = TokenService::hash_token(refresh_token);
        self.repo.revoke_refresh_token(&hash).await
    }

    pub async fn logout_all(&self, user_id: Uuid) -> Result<(), AuthError> {
        self.repo.revoke_all_user_tokens(user_id).await
    }

    async fn issue_tokens(&self, user: &User) -> Result<AuthTokens, AuthError> {
        let access = self.tokens.issue_access_token(
            user.id.0,
            user.organization_id.0,
            &user.role.to_string(),
        )?;

        let (refresh, hash, expires_at) = self.tokens.issue_refresh_token();

        self.repo
            .store_refresh_token(user.id.0, &hash, expires_at)
            .await?;

        Ok(AuthTokens {
            access_token: access,
            refresh_token: refresh,
            expires_in: self.tokens.access_ttl_secs(),
            token_type: "Bearer".to_string(),
        })
    }
}
