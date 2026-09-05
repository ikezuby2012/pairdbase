use async_trait::async_trait;
use chrono::{DateTime, Utc};
use shared::ids::{UserId};
use sqlx;
use uuid::Uuid;

use crate::db::DbPool;
use crate::features::auth::auth_error::AuthError;
use crate::features::auth::domains::{
    NewUser, OAuthAccount, OAuthProvider, OAuthState, User, UserRow,
};

#[async_trait]
pub trait AuthRepo: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError>;
    async fn find_user_by_id(&self, id: Uuid) -> Result<Option<User>, AuthError>;
    async fn create_user(&self, user: NewUser) -> Result<User, AuthError>;
    async fn update_last_login(&self, user_id: Uuid) -> Result<(), AuthError>;
    async fn find_oauth_account(
        &self,
        provider: &OAuthProvider,
        provider_user_id: &str,
    ) -> Result<Option<OAuthAccount>, AuthError>;

    async fn create_oauth_account(
        &self,
        user_id: Uuid,
        provider: &OAuthProvider,
        provider_user_id: &str,
    ) -> Result<(), AuthError>;

    // Refresh tokens
    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError>;

    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<Uuid>, AuthError>;
    async fn revoke_refresh_token(&self, token_hash: &str) -> Result<(), AuthError>;
    async fn revoke_all_user_tokens(&self, user_id: Uuid) -> Result<(), AuthError>;

    // OAuth PKCE state
    async fn store_oauth_state(
        &self,
        state: &str,
        provider: &str,
        code_verifier: &str,
        redirect_to: Option<&str>,
    ) -> Result<(), AuthError>;

    async fn consume_oauth_state(&self, state: &str) -> Result<Option<OAuthState>, AuthError>;
}

pub struct PgAuthRepo {
    pool: DbPool,
}

impl PgAuthRepo {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthRepo for PgAuthRepo {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        sqlx::query_as!(
            UserRow,
            r#"
        SELECT
            id,
            organization_id,
            email,
            display_name,
            avatar_url,
            password_hash,
            role,
            is_verified,
            last_login_at,
            created_at,
            updated_at,
            updated_by,
            is_soft_deleted,
            deleted_at
        FROM tbl_users
        WHERE email = $1
          AND is_soft_deleted = FALSE
        LIMIT 1
        "#,
            email
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))
        .map(|r| r.map(Into::into))
    }

    async fn find_user_by_id(&self, id: Uuid) -> Result<Option<User>, AuthError> {
        sqlx::query_as!(
            UserRow,
            r#"
        SELECT
            id,
            organization_id,
            email,
            display_name,
            avatar_url,
            password_hash,
            role,
            is_verified,
            last_login_at,
            created_at,
            updated_at,
            updated_by,
            is_soft_deleted,
            deleted_at
        FROM tbl_users
        WHERE id = $1
          AND is_soft_deleted = FALSE
        LIMIT 1
        "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))
        .map(|r| r.map(Into::into))
    }

    async fn create_user(&self, new: NewUser) -> Result<User, AuthError> {
        sqlx::query_as!(
            UserRow,
            r#"
        INSERT INTO tbl_users (
            organization_id,
            email,
            display_name,
            avatar_url,
            password_hash,
            is_verified
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING
            id,
            organization_id,
            email,
            display_name,
            avatar_url,
            password_hash,
            role,
            is_verified,
            last_login_at,
            created_at,
            updated_at,
            updated_by,
            is_soft_deleted,
            deleted_at
        "#,
            new.organization_id,
            new.email,
            new.display_name,
            new.avatar_url,
            new.password_hash,
            new.is_verified,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db)
                if db.constraint() == Some("users_organization_id_email_key") =>
            {
                AuthError::EmailTaken
            }

            other => AuthError::Internal(other.to_string()),
        })
        .map(Into::into)
    }

    async fn update_last_login(&self, user_id: Uuid) -> Result<(), AuthError> {
        sqlx::query!(
            "UPDATE tbl_users SET last_login_at = NOW() WHERE id = $1",
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    // ── OAuth accounts ────────────────────────────────────────────────────────

    async fn find_oauth_account(
        &self,
        provider: &OAuthProvider,
        provider_user_id: &str,
    ) -> Result<Option<OAuthAccount>, AuthError> {
        let row = sqlx::query!(
            r#"
            SELECT id, user_id, provider, provider_user_id
            FROM tbl_oauth_accounts
            WHERE provider = $1 AND provider_user_id = $2
            "#,
            provider.as_str(),
            provider_user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(row.map(|r| OAuthAccount {
            id: r.id,
            user_id: UserId(r.user_id),
            provider: provider.clone(),
            provider_user_id: r.provider_user_id,
        }))
    }

    async fn create_oauth_account(
        &self,
        user_id: Uuid,
        provider: &OAuthProvider,
        provider_user_id: &str,
    ) -> Result<(), AuthError> {
        sqlx::query!(
            r#"
            INSERT INTO tbl_oauth_accounts (user_id, provider, provider_user_id)
            VALUES ($1, $2, $3)
            ON CONFLICT (provider, provider_user_id) DO NOTHING
            "#,
            user_id,
            provider.as_str(),
            provider_user_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    // ── Refresh tokens ────────────────────────────────────────────────────────

    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        sqlx::query!(
            r#"
            INSERT INTO tbl_refresh_tokens (user_id, token_hash, expires_at)
            VALUES ($1, $2, $3)
            "#,
            user_id,
            token_hash,
            expires_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<Uuid>, AuthError> {
        let row = sqlx::query!(
            r#"
            SELECT user_id FROM tbl_refresh_tokens
            WHERE token_hash = $1
              AND revoked_at IS NULL
              AND expires_at > NOW()
            "#,
            token_hash,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(row.map(|r| r.user_id))
    }

    async fn revoke_refresh_token(&self, token_hash: &str) -> Result<(), AuthError> {
        sqlx::query!(
            r#"
            UPDATE tbl_refresh_tokens
            SET revoked_at = NOW()
            WHERE token_hash = $1 AND revoked_at IS NULL
            "#,
            token_hash,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn revoke_all_user_tokens(&self, user_id: Uuid) -> Result<(), AuthError> {
        sqlx::query!(
            r#"
            UPDATE tbl_refresh_tokens
            SET revoked_at = NOW()
            WHERE user_id = $1 AND revoked_at IS NULL
            "#,
            user_id,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    // ── OAuth PKCE state ──────────────────────────────────────────────────────

    async fn store_oauth_state(
        &self,
        state: &str,
        provider: &str,
        code_verifier: &str,
        redirect_to: Option<&str>,
    ) -> Result<(), AuthError> {
        sqlx::query!(
            r#"
            INSERT INTO tbl_oauth_states (state, provider, code_verifier, redirect_to)
            VALUES ($1, $2, $3, $4)
            "#,
            state,
            provider,
            code_verifier,
            redirect_to,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn consume_oauth_state(&self, state: &str) -> Result<Option<OAuthState>, AuthError> {
        // DELETE and return in one query — prevents double-use
        let row = sqlx::query!(
            r#"
            DELETE FROM tbl_oauth_states
            WHERE state = $1 AND expires_at > NOW()
            RETURNING provider, code_verifier, redirect_to
            "#,
            state,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Internal(e.to_string()))?;

        Ok(row.map(|r| OAuthState {
            provider: r.provider,
            code_verifier: r.code_verifier,
            redirect_to: r.redirect_to,
        }))
    }
}
