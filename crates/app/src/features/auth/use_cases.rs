use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;
use shared::{UserId, OrgId};
// use rand_core::{OsRng, RngCore};

use crate::{
    abstractions::helpers::AuditInfo,
    features::{
        auth::{
            auth_error::AuthError,
            domains::{AuthTokens, NewUser, OAuthProfile, User, OAuthAccount},
            repository::AuthRepo,
            tokens::TokenService,
        },
        organization::repository::OrgRepo,
        workspace::use_case::WorkspaceUseCases,
    },
    services::outbox::{events::OutboxEvent, OutboxPublisher},
};

pub struct AuthUseCases {
    repo: Arc<dyn AuthRepo>,
    tokens: Arc<TokenService>,
    org_repo: Arc<dyn OrgRepo>,
    workspace_uc: Arc<WorkspaceUseCases>,
}

impl AuthUseCases {
    pub fn new(
        repo: Arc<dyn AuthRepo>,
        tokens: Arc<TokenService>,
        org_repo: Arc<dyn OrgRepo>,
        workspace_uc: Arc<WorkspaceUseCases>,
    ) -> Self {
        Self {
            repo,
            tokens,
            org_repo,
            workspace_uc,
        }
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

    async fn register_new_oauth_user(
        &self,
        profile: OAuthProfile,
    ) -> Result<AuthTokens, AuthError> {
        let user_id = Uuid::new_v4();
        let org_id = Uuid::new_v4();
        let ws_id = Uuid::new_v4();
        let now = Utc::now();

        // Derive a clean org slug from the display name
        let org_name = format!("{}'s Workspace", profile.display_name);
        let org_slug = slugify(&profile.display_name);

        // ── All in one transaction ────────────────────────────────────────────
        let mut tx = self.repo.begin().await?;

        // 1. Create user
        let user = User {
            id: UserId(user_id),
            organization_id: OrgId(org_id),
            email: profile.email.clone(),
            display_name: profile.display_name.clone(),
            avatar_url: profile.avatar_url.clone(),
            password_hash: None, // OAuth-only user — no password
            audit: AuditInfo {
                created_at: now,
                updated_at: Some(now),
                deleted_at: None,
                is_soft_deleted: false,
                updated_by: None
            }, // email_verified: true, // provider already verified
        };
        self.repo.create_user_in_tx(&mut tx, &user).await?;

        // 2. Link OAuth account
        self.repo
            .create_oauth_account_in_tx(
                &mut tx,
                OAuthAccount {
                    id: Uuid::new_v4(),
                    user_id,
                    provider: profile.provider.as_str().to_string(),
                    provider_user_id: profile.provider_user_id.clone(),
                    email: profile.email.clone(),
                    created_at: now,
                },
            )
            .await?;

        // 3. Create personal organization
        let org = Organization {
            id: org_id,
            name: org_name.clone(),
            slug: org_slug,
            plan: OrgPlan::Free,
            logo_url: None,
            website: None,
            sso_enabled: false,
            max_workspaces: OrgPlan::Free.max_workspaces(),
            max_members: OrgPlan::Free.max_members(),
            max_connections: OrgPlan::Free.max_connections(),
            ai_credits_limit: OrgPlan::Free.ai_credits_limit(),
            ai_credits_used: 0,
            trial_ends_at: None,
            created_at: now,
            updated_at: now,
        };
        self.org_repo.create_in_tx(&mut tx, &org).await?;

        // 4. Add user as org owner
        self.org_repo
            .add_member_in_tx(&mut tx, org_id, user_id, &OrgRole::Owner, None)
            .await?;

        // 5. Create default workspace
        self.workspace_uc
            .repo
            .create_in_tx(&mut tx, ws_id, org_id, user_id, "My Workspace")
            .await?;

        // 6. Add user as workspace owner
        self.workspace_uc
            .repo
            .add_member_in_tx(&mut tx, ws_id, user_id, &MemberRole::Owner, user_id)
            .await?;

        // 7. Enqueue welcome email
        OutboxPublisher::publish_in_tx(
            &OutboxEvent::EmailWelcome {
                to: profile.email.clone(),
                display_name: profile.display_name.clone(),
                verify_token: String::new(), // already verified via OAuth
            },
            &mut tx,
        )
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| AuthError::Internal(e.to_string()))?;

        tracing::info!(
            user_id = %user_id,
            org_id  = %org_id,
            ws_id   = %ws_id,
            email   = %profile.email,
            "new OAuth user registered"
        );

        self.issue_tokens(&user)
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

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
