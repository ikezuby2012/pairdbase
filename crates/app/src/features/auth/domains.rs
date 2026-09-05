use crate::abstractions::helpers::AuditInfo;
use crate::features::auth::auth_error::AuthError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::ids::{OrgId, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub organization_id: OrgId,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub role: Role,
    pub is_verified: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub audit: AuditInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Owner,
    Admin,
    Member,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Role::Owner => write!(f, "owner"),
            Role::Admin => write!(f, "admin"),
            Role::Member => write!(f, "member"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OAuthProvider {
    Google,
    GitHub,
    Twitter,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::GitHub => "github",
            OAuthProvider::Twitter => "twitter",
            // OAuthProvider::Facebook => "facebook",
        }
    }
}

impl TryFrom<&str> for OAuthProvider {
    type Error = AuthError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "google" => Ok(OAuthProvider::Google),
            "github" => Ok(OAuthProvider::GitHub),
            "twitter" => Ok(OAuthProvider::Twitter),
            // "facebook" => Ok(OAuthProvider::Facebook),
            other => Err(AuthError::UnsupportedProvider(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthAccount {
    pub id: Uuid,
    pub user_id: UserId,
    pub provider: OAuthProvider,
    pub provider_user_id: String,
}

/// The profile returned by any OAuth provider after successful login
#[derive(Debug, Clone)]
pub struct OAuthProfile {
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// Issued tokens returned to the client
#[derive(Debug, Serialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub role: String,
    pub is_verified: bool,
    pub last_login_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub updated_by: Option<Uuid>,
    pub is_soft_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: UserId(row.id),
            organization_id: OrgId(row.organization_id),
            email: row.email,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
            password_hash: row.password_hash,
            role: match row.role.as_str() {
                "owner" => Role::Owner,
                "admin" => Role::Admin,
                _ => Role::Member,
            },
            is_verified: row.is_verified,
            last_login_at: row.last_login_at,

            audit: AuditInfo {
                created_at: row.created_at,
                updated_at: row.updated_at,
                updated_by: row.updated_by,
                is_soft_deleted: row.is_soft_deleted,
                deleted_at: row.deleted_at,
            },
        }
    }
}

#[derive(Debug)]
pub struct NewUser {
    pub organization_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub is_verified: bool,
}

#[derive(Debug)]
pub struct OAuthState {
    pub provider: String,
    pub code_verifier: String,
    pub redirect_to: Option<String>,
}

/// Token response shared across providers
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}
