use super::error::WorkspaceError;
use crate::features::auth::domains::User;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shared::{ConnectionId, OrgId, UserId, WorkspaceId, WorkspaceMemberId};
use sqlx::FromRow;
use uuid::Uuid;

// ── Entities ──────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UserSummary {
    pub id: UserId,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub organization_id: OrgId,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceMemberRow {
    pub id: Uuid,
    pub workspace_id: Uuid,
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

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub organization_id: OrgId,

    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkspaceMember {
    pub id: WorkspaceMemberId,
    pub workspace_id: WorkspaceId,
    pub user_id: UserId,

    pub role: MemberRole,
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
pub struct WorkspaceMemberWithUser {
    pub member: WorkspaceMember,
    pub user: UserSummary,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceMemberWithUserRow {
    // workspace member
    pub member_id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,

    // audit
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,

    // user
    pub user_email: String,
    pub user_display_name: String,
    pub user_avatar_url: Option<String>,
    pub user_organization_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl MemberRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemberRole::Owner => "owner",
            MemberRole::Admin => "admin",
            MemberRole::Member => "member",
            MemberRole::Viewer => "viewer",
        }
    }

    pub fn can_manage_members(&self) -> bool {
        matches!(self, MemberRole::Owner | MemberRole::Admin)
    }

    pub fn can_manage_connections(&self) -> bool {
        matches!(
            self,
            MemberRole::Owner | MemberRole::Admin | MemberRole::Member
        )
    }

    pub fn can_execute_queries(&self) -> bool {
        matches!(
            self,
            MemberRole::Owner | MemberRole::Admin | MemberRole::Member
        )
    }
}

impl TryFrom<&str> for MemberRole {
    type Error = WorkspaceError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            "viewer" => Ok(Self::Viewer),
            other => Err(WorkspaceError::InvalidRole(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceView {
    pub id: Uuid,
    pub organization_id: Uuid,

    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,

    pub member_count: i64,
    pub user_role: String,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkspaceMemberView {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub user_id: Uuid,

    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,

    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl TryFrom<WorkspaceRow> for Workspace {
    type Error = String;

    fn try_from(row: WorkspaceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: WorkspaceId(row.id),
            organization_id: OrgId(row.organization_id),

            name: row.name,
            description: row.description,
            color: row.color,

            created_by: row.created_by,
            created_at: row.created_at,
            updated_by: row.updated_by,
            updated_at: row.updated_at,

            deleted_at: row.deleted_at,
            deleted_by: row.deleted_by,
            is_soft_deleted: row.is_soft_deleted,
        })
    }
}

impl TryFrom<WorkspaceMemberRow> for WorkspaceMember {
    type Error = WorkspaceError;

    fn try_from(row: WorkspaceMemberRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: WorkspaceMemberId(row.id),
            workspace_id: WorkspaceId(row.workspace_id),
            user_id: UserId(row.user_id),

            role: MemberRole::try_from(row.role.as_str())?,
            joined_at: row.joined_at,

            created_by: row.created_by,
            created_at: row.created_at,
            updated_by: row.updated_by,
            updated_at: row.updated_at,

            deleted_at: row.deleted_at,
            deleted_by: row.deleted_by,
            is_soft_deleted: row.is_soft_deleted,
        })
    }
}

impl From<Workspace> for WorkspaceView {
    fn from(workspace: Workspace) -> Self {
        Self {
            id: workspace.id.0,
            organization_id: workspace.organization_id.0,

            name: workspace.name,
            description: workspace.description,
            color: workspace.color,

            created_by: workspace.created_by,
            created_at: workspace.created_at,
            updated_by: workspace.updated_by,
            updated_at: workspace.updated_at,

            member_count: 0,
            user_role: "".to_string(),
        }
    }
}

impl TryFrom<WorkspaceMemberWithUserRow> for WorkspaceMemberWithUser {
    type Error = WorkspaceError;

    fn try_from(row: WorkspaceMemberWithUserRow) -> Result<Self, Self::Error> {
        Ok(Self {
            member: WorkspaceMember {
                id: WorkspaceMemberId(row.member_id),
                workspace_id: WorkspaceId(row.workspace_id),
                user_id: UserId(row.user_id),

                role: MemberRole::try_from(row.role.as_str())?,
                joined_at: row.joined_at,

                created_by: row.created_by,
                created_at: row.created_at,
                updated_by: row.updated_by,
                updated_at: row.updated_at,

                is_soft_deleted: row.is_soft_deleted,
                deleted_by: row.deleted_by,
                deleted_at: row.deleted_at,
            },

            user: UserSummary {
                id: UserId(row.user_id),
                email: row.user_email,
                display_name: row.user_display_name,
                avatar_url: row.user_avatar_url,
                organization_id: OrgId(row.user_organization_id),
            },
        })
    }
}
