use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::{OrgId, OrgInviteId, OrgMemberId, UserId};
use sqlx::FromRow;
use uuid::Uuid;

use super::error::OrgError;

// ── Plan ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrgPlan {
    Free,
    Pro,
    Team,
    Enterprise,
}

impl OrgPlan {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgPlan::Free => "free",
            OrgPlan::Pro => "pro",
            OrgPlan::Team => "team",
            OrgPlan::Enterprise => "enterprise",
        }
    }

    pub fn max_workspaces(&self) -> i32 {
        match self {
            OrgPlan::Free => 3,
            OrgPlan::Pro => 20,
            OrgPlan::Team => 100,
            OrgPlan::Enterprise => i32::MAX,
        }
    }

    pub fn max_members(&self) -> i32 {
        match self {
            OrgPlan::Free => 5,
            OrgPlan::Pro => 10,
            OrgPlan::Team => 100,
            OrgPlan::Enterprise => i32::MAX,
        }
    }

    pub fn max_connections(&self) -> i32 {
        match self {
            OrgPlan::Free => 10,
            OrgPlan::Pro => 50,
            OrgPlan::Team => 200,
            OrgPlan::Enterprise => i32::MAX,
        }
    }

    pub fn ai_credits_limit(&self) -> i32 {
        match self {
            OrgPlan::Free => 50,
            OrgPlan::Pro => 500,
            OrgPlan::Team => 2000,
            OrgPlan::Enterprise => i32::MAX,
        }
    }
}

impl TryFrom<&str> for OrgPlan {
    type Error = OrgError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "free" => Ok(OrgPlan::Free),
            "pro" => Ok(OrgPlan::Pro),
            "team" => Ok(OrgPlan::Team),
            "enterprise" => Ok(OrgPlan::Enterprise),
            other => Err(OrgError::InvalidPlan(other.to_string())),
        }
    }
}

// ── Member role ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrgRole {
    Owner,
    Admin,
    Member,
}

impl OrgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgRole::Owner => "owner",
            OrgRole::Admin => "admin",
            OrgRole::Member => "member",
        }
    }

    pub fn can_manage_members(&self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Admin)
    }

    pub fn can_manage_billing(&self) -> bool {
        matches!(self, OrgRole::Owner)
    }

    pub fn can_manage_sso(&self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Admin)
    }

    pub fn can_view_audit_log(&self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Admin)
    }
}

impl TryFrom<&str> for OrgRole {
    type Error = OrgError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "owner" => Ok(OrgRole::Owner),
            "admin" => Ok(OrgRole::Admin),
            "member" => Ok(OrgRole::Member),
            other => Err(OrgError::InvalidRole(other.to_string())),
        }
    }
}

// ── Entities ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Organization {
    pub id: OrgId,
    pub name: String,
    pub slug: String,
    pub plan: OrgPlan,
    pub logo_url: Option<String>,
    pub website: Option<String>,
    pub sso_enabled: bool,
    pub max_workspaces: i32,
    pub max_members: i32,
    pub max_connections: i32,
    pub ai_credits_limit: i32,
    pub ai_credits_used: i32,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct OrganizationRow {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub logo_url: Option<String>,
    pub website: Option<String>,
    pub sso_enabled: bool,
    pub max_workspaces: i32,
    pub max_members: i32,
    pub max_connections: i32,
    pub ai_credits_limit: i32,
    pub ai_credits_used: i32,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,

    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<OrganizationRow> for Organization {
    type Error = OrgError;

    fn try_from(row: OrganizationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: OrgId(row.id),
            name: row.name,
            slug: row.slug,
            plan: OrgPlan::try_from(row.plan.as_str())?,
            logo_url: row.logo_url,
            website: row.website,
            sso_enabled: row.sso_enabled,
            max_workspaces: row.max_workspaces,
            max_members: row.max_members,
            max_connections: row.max_connections,
            ai_credits_limit: row.ai_credits_limit,
            ai_credits_used: row.ai_credits_used,
            trial_ends_at: row.trial_ends_at,
            created_at: row.created_at,
            updated_at: row.updated_at.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrganizationView {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: OrgPlan,
    pub logo_url: Option<String>,
    pub website: Option<String>,
    pub sso_enabled: bool,
    pub max_workspaces: i32,
    pub max_members: i32,
    pub max_connections: i32,
    pub ai_credits_limit: i32,
    pub ai_credits_used: i32,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Organization> for OrganizationView {
    fn from(org: Organization) -> Self {
        Self {
            id: org.id.0,
            name: org.name,
            slug: org.slug,
            plan: org.plan,
            logo_url: org.logo_url,
            website: org.website,
            sso_enabled: org.sso_enabled,
            max_workspaces: org.max_workspaces,
            max_members: org.max_members,
            max_connections: org.max_connections,
            ai_credits_limit: org.ai_credits_limit,
            ai_credits_used: org.ai_credits_used,
            trial_ends_at: org.trial_ends_at,
            created_at: org.created_at,
            updated_at: org.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct OrgMember {
    pub id: OrgMemberId,
    pub org_id: OrgId,
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: OrgRole,
    pub joined_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgMemberRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub created_by: Uuid,

    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrgMemberView {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: OrgRole,
    pub joined_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema, sqlx::FromRow, sqlx::Decode)]
pub struct OrgMemberWithUserView {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

// impl TryFrom<OrgMember> for OrgMemberWithUserView {
//     type Error = OrgError;

//     fn try_from(member: OrgMember) -> Result<Self, Self::Error> {
//         Ok(Self {
//             id: member.id.0,
//             org_id: member.org_id.0,
//             user_id: member.user_id.0,
//             email: member.email,
//             display_name: member.display_name,
//             avatar_url: member.avatar_url,
//             role: member.role.as_str().to_string(),
//             joined_at: member.joined_at,
//             created_at: member.created_at,
//             created_by: member.created_by,
//             updated_at: member.updated_at,
//             updated_by: member.updated_by,
//         })
//     }
// }

impl TryFrom<OrgMemberWithUserView> for OrgMember {
    type Error = OrgError;

    fn try_from(member: OrgMemberWithUserView) -> Result<Self, Self::Error> {
        Ok(Self {
            id: OrgMemberId(member.id),
            org_id: OrgId(member.org_id),
            user_id: UserId(member.user_id),
            email: member.email,
            display_name: member.display_name,
            avatar_url: member.avatar_url,
            role: OrgRole::try_from(member.role.as_str())?,
            joined_at: member.joined_at,
            created_at: member.created_at,
            created_by: member.created_by,
            updated_at: member.updated_at,
            updated_by: member.updated_by,
        })
    }
}

impl From<OrgMember> for OrgMemberWithUserView {
    fn from(member: OrgMember) -> Self {
        Self {
            id: member.id.0,
            org_id: member.org_id.0,
            user_id: member.user_id.0,
            email: member.email,
            display_name: member.display_name,
            avatar_url: member.avatar_url,
            role: member.role.as_str().to_string(),
            joined_at: member.joined_at,
            created_at: member.created_at,
            created_by: member.created_by,
            updated_at: member.updated_at,
            updated_by: member.updated_by,
        }
    }
}

impl From<OrgMember> for OrgMemberView {
    fn from(member: OrgMember) -> Self {
        Self {
            id: member.id.0,
            org_id: member.org_id.0,
            user_id: member.user_id.0,
            email: member.email,
            display_name: member.display_name,
            avatar_url: member.avatar_url,
            role: member.role,
            joined_at: member.joined_at,
            created_at: member.created_at,
            created_by: member.created_by,
            updated_at: member.updated_at,
            updated_by: member.updated_by,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct OrgInvitation {
    pub id: OrgInviteId,
    pub org_id: OrgId,
    pub email: String,
    pub role: OrgRole,
    pub invited_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_by: Option<Uuid>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgInvitationRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub role: String,
    pub invited_by: Uuid,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_by: Option<Uuid>,
    pub expires_at: DateTime<Utc>,
    pub created_by: Uuid,

    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl TryFrom<OrgInvitationRow> for OrgInvitation {
    type Error = OrgError;

    fn try_from(row: OrgInvitationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: OrgInviteId(row.id),
            org_id: OrgId(row.org_id),
            email: row.email,
            role: OrgRole::try_from(row.role.as_str())?,
            invited_by: row.invited_by,
            expires_at: row.expires_at,
            created_at: row.created_at,
            accepted_at: row.accepted_at,
            accepted_by: row.accepted_by,
        })
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrgInvitationView {
    pub id: Uuid,
    pub org_id: Uuid,
    pub email: String,
    pub role: OrgRole,
    pub invited_by: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<OrgInvitation> for OrgInvitationView {
    fn from(invitation: OrgInvitation) -> Self {
        Self {
            id: invitation.id.0,
            org_id: invitation.org_id.0,
            email: invitation.email,
            role: invitation.role,
            invited_by: invitation.invited_by,
            expires_at: invitation.expires_at,
            created_at: invitation.created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrgAuditLog {
    pub id: i64,
    pub org_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_type: String,
    pub event_type: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OrgAuditLogRow {
    pub id: i64,
    pub org_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_type: String,
    pub event_type: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_by: Uuid,

    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl From<OrgAuditLogRow> for OrgAuditLog {
    fn from(row: OrgAuditLogRow) -> Self {
        Self {
            id: row.id,
            org_id: row.org_id,
            actor_id: row.actor_id,
            actor_type: row.actor_type,
            event_type: row.event_type,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            metadata: row.metadata,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrgAuditLogView {
    pub id: i64,
    pub org_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_type: String,
    pub event_type: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrgUsageView {
    pub workspace_count: i64,
    pub member_count: i64,
    pub connection_count: i64,
    pub ai_credits_used: i32,
    pub ai_credits_limit: i32,
    pub limits: OrgLimitsView,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OrgLimitsView {
    pub max_workspaces: i32,
    pub max_members: i32,
    pub max_connections: i32,
}
