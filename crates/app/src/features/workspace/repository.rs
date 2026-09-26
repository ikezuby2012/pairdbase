use async_trait::async_trait;
use shared::WorkspaceId;
use uuid::Uuid;

use crate::db::DbPool;

use super::domain::{
    MemberRole, Workspace, WorkspaceMember, WorkspaceMemberWithUser, WorkspaceMemberWithUserRow,
    WorkspaceRow, WorkspaceView, WorkspaceWithOrganization, WorkspaceWithOrganizationRow
};
use super::error::WorkspaceError;

#[async_trait]
pub trait WorkspaceRepo: Send + Sync {
    async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, WorkspaceError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Workspace>, WorkspaceError>;

    async fn list_for_user(
        &self,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Vec<WorkspaceView>, WorkspaceError>;

    async fn create(&self, workspace: Workspace) -> Result<Workspace, WorkspaceError>;

    async fn update(&self, workspace: Workspace) -> Result<Workspace, WorkspaceError>;

    async fn soft_delete(&self, id: Uuid) -> Result<(), WorkspaceError>;

    // Members
    async fn get_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<MemberRole>, WorkspaceError>;

    async fn list_members(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMemberWithUser>, WorkspaceError>;

    async fn get_member(
        &self,
        user_id: Uuid,
    ) -> Result<Option<WorkspaceMemberWithUser>, WorkspaceError>;

    async fn get_with_organization(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Option<WorkspaceWithOrganization>, WorkspaceError>;

    async fn find_user_by_email(
        &self,
        email: &str,
        org_id: Uuid,
    ) -> Result<Option<Uuid>, WorkspaceError>;

    async fn add_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &MemberRole,
        invited_by: Uuid,
    ) -> Result<(), WorkspaceError>;

    async fn update_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &MemberRole,
    ) -> Result<(), WorkspaceError>;

    async fn remove_member(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), WorkspaceError>;

    async fn member_count(&self, workspace_id: Uuid) -> Result<i64, WorkspaceError>;
}

pub struct PgWorkspaceRepo {
    pool: DbPool,
}

impl PgWorkspaceRepo {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WorkspaceRepo for PgWorkspaceRepo {
    async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, WorkspaceError> {
        self.pool
            .begin()
            .await
            .map_err(|e| WorkspaceError::Internal(e.to_string()))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Workspace>, WorkspaceError> {
        let row = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            SELECT
                id,
                organization_id,
                name,
                description,
                color,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_WORKSPACES
            WHERE id = $1
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        row.map(|row| Workspace::try_from(row).map_err(WorkspaceError::Internal))
            .transpose()
    }

    async fn list_for_user(
        &self,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Vec<WorkspaceView>, WorkspaceError> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            SELECT
                w.id,
                w.organization_id,
                w.name,
                w.description,
                w.color,
                w.created_by,
                w.created_at,
                w.updated_by,
                w.updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_WORKSPACES w
            INNER JOIN TBL_WORKSPACE_MEMBERS wm
                ON wm.workspace_id = w.id
            WHERE w.organization_id = $1
              AND wm.user_id = $2
              AND wm.is_soft_deleted = FALSE
              AND wm.deleted_at IS NULL
              AND w.is_soft_deleted = FALSE
              AND w.deleted_at IS NULL
            ORDER BY w.created_at DESC
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        let workspaces = rows
            .into_iter()
            .map(|row| Workspace::try_from(row).map_err(WorkspaceError::Internal))
            .map(|result| result.map(WorkspaceView::from))
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        Ok(workspaces)
    }

    async fn create(&self, workspace: Workspace) -> Result<Workspace, WorkspaceError> {
        let row = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            INSERT INTO TBL_WORKSPACES (
                id,
                organization_id,
                name,
                description,
                color,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                FALSE,
                NULL,
                NULL
            )
            RETURNING
                id,
                organization_id,
                name,
                description,
                color,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            "#,
        )
        .bind(workspace.id.0)
        .bind(workspace.organization_id.0)
        .bind(&workspace.name)
        .bind(&workspace.description)
        .bind(&workspace.color)
        .bind(workspace.created_by)
        .bind(workspace.created_at)
        .bind(workspace.updated_by)
        .bind(workspace.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                WorkspaceError::NameTaken
            } else {
                WorkspaceError::Internal(e.to_string())
            }
        })?;

        Workspace::try_from(row).map_err(WorkspaceError::Internal)
    }

    async fn update(&self, workspace: Workspace) -> Result<Workspace, WorkspaceError> {
        let row = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            UPDATE TBL_WORKSPACES
            SET
                name = $2,
                description = $3,
                color = $4,
                updated_by = $5,
                updated_at = $6
            WHERE id = $1
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            RETURNING
                id,
                organization_id,
                name,
                description,
                color,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            "#,
        )
        .bind(workspace.id.0)
        .bind(&workspace.name)
        .bind(&workspace.description)
        .bind(&workspace.color)
        .bind(workspace.updated_by)
        .bind(workspace.updated_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                WorkspaceError::NameTaken
            } else {
                WorkspaceError::Internal(e.to_string())
            }
        })?;

        let row = row.ok_or(WorkspaceError::NotFound)?;

        Workspace::try_from(row).map_err(WorkspaceError::Internal)
    }

    async fn soft_delete(&self, id: Uuid) -> Result<(), WorkspaceError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_WORKSPACES
            SET
                is_soft_deleted = TRUE,
                deleted_at = NOW()
            WHERE id = $1
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(WorkspaceError::NotFound);
        }

        Ok(())
    }

    // ---------------------------------------------------------------------
    // Members
    // ---------------------------------------------------------------------

    async fn get_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<MemberRole>, WorkspaceError> {
        let role: Option<String> = sqlx::query_scalar(
            r#"
            SELECT role
            FROM TBL_WORKSPACE_MEMBERS
            WHERE workspace_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        role.map(|role| {
            MemberRole::try_from(role.as_str()).map_err(|e| WorkspaceError::Internal(e.to_string()))
        })
        .transpose()
    }

    async fn list_members(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMemberWithUser>, WorkspaceError> {
        let rows = sqlx::query_as::<_, WorkspaceMemberWithUserRow>(
            r#"
            SELECT
                wm.id AS member_id,
                wm.workspace_id,
                wm.user_id,
                wm.role,
                wm.joined_at,
                wm.created_by AS created_by,
                wm.created_at AS created_at,
                wm.updated_by AS updated_by,
                wm.updated_at AS updated_at,
                wm.is_soft_deleted AS is_soft_deleted,
                wm.deleted_by AS deleted_by,
                wm.deleted_at AS deleted_at,

                u.email AS user_email,
                u.display_name AS user_display_name,
                u.avatar_url AS user_avatar_url
            FROM TBL_WORKSPACE_MEMBERS wm
            INNER JOIN TBL_USERS u
                ON u.id = wm.user_id
            WHERE wm.workspace_id = $1
              AND wm.is_soft_deleted = FALSE
              AND wm.deleted_at IS NULL
            ORDER BY wm.joined_at ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        let members = rows
            .into_iter()
            .map(WorkspaceMemberWithUser::try_from)
            .collect::<Result<Vec<_>, WorkspaceError>>()?;

        Ok(members)
    }

    async fn get_member(
        &self,
        user_id: Uuid,
    ) -> Result<Option<WorkspaceMemberWithUser>, WorkspaceError> {
        let row = sqlx::query_as::<_, WorkspaceMemberWithUserRow>(
            r#"
            SELECT
                wm.id AS member_id,
                wm.workspace_id,
                wm.user_id,
                wm.role,
                wm.joined_at,
                wm.created_by AS created_by,
                wm.created_at AS created_at,
                wm.updated_by AS updated_by,
                wm.updated_at AS updated_at,
                wm.is_soft_deleted AS is_soft_deleted,
                wm.deleted_by AS deleted_by,
                wm.deleted_at AS deleted_at,

                u.email AS user_email,
                u.display_name AS user_display_name,
                u.avatar_url AS user_avatar_url
            FROM TBL_WORKSPACE_MEMBERS wm
            INNER JOIN TBL_USERS u
                ON u.id = wm.user_id
            WHERE wm.user_id = $1
              AND wm.is_soft_deleted = FALSE
              AND wm.deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        let member = row.map(WorkspaceMemberWithUser::try_from).transpose()?;

        Ok(member)
    }

    async fn get_with_organization(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Option<WorkspaceWithOrganization>, WorkspaceError> {
        let row = sqlx::query_as::<_, WorkspaceWithOrganizationRow>(
            r#"
        SELECT
            w.id AS workspace_id,
            w.organization_id AS organization_id,
            w.name AS workspace_name,
            w.description AS workspace_description,
            w.color AS workspace_color,
            w.created_by AS workspace_created_by,
            w.created_at AS workspace_created_at,
            w.updated_by AS workspace_updated_by,
            w.updated_at AS workspace_updated_at,

            o.name AS organization_name,
            o.created_by AS organization_created_by

        FROM TBL_WORKSPACES w

        INNER JOIN TBL_ORGANIZATIONS o
            ON o.id = w.organization_id

        WHERE w.id = $1
          AND w.is_soft_deleted = FALSE
          AND w.deleted_at IS NULL
        "#,
        )
        .bind(workspace_id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(
                error = ?e,
                workspace_id = %workspace_id.0,
                "Failed to fetch workspace with organization"
            );

            WorkspaceError::Internal(e.to_string())
        })?;

        Ok(row.map(WorkspaceWithOrganization::from))
    }

    async fn find_user_by_email(
        &self,
        email: &str,
        org_id: Uuid,
    ) -> Result<Option<Uuid>, WorkspaceError> {
        let user_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM TBL_USERS
            WHERE LOWER(email) = LOWER($1)
              AND organization_id = $2
            LIMIT 1
            "#,
        )
        .bind(email.trim())
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        Ok(user_id)
    }

    async fn add_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &MemberRole,
        invited_by: Uuid,
    ) -> Result<(), WorkspaceError> {
        sqlx::query(
            r#"
            INSERT INTO TBL_WORKSPACE_MEMBERS (
                id,
                workspace_id,
                user_id,
                role,
                joined_at,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            )
            VALUES (
                gen_random_uuid(),
                $1,
                $2,
                $3,
                NOW(),
                $4,
                NOW(),
                NULL,
                NULL,
                FALSE,
                NULL,
                NULL
            )
            "#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role.as_str())
        .bind(invited_by)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                WorkspaceError::AlreadyMember
            } else {
                WorkspaceError::Internal(e.to_string())
            }
        })?;

        Ok(())
    }

    async fn update_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: &MemberRole,
    ) -> Result<(), WorkspaceError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_WORKSPACE_MEMBERS
            SET
                role = $3,
                updated_at = NOW()
            WHERE workspace_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(WorkspaceError::UserNotFound(
                "workspace member not found".to_string(),
            ));
        }

        Ok(())
    }

    async fn remove_member(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), WorkspaceError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_WORKSPACE_MEMBERS
            SET
                is_soft_deleted = TRUE,
                deleted_at = NOW()
            WHERE workspace_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
              AND role <> 'owner'
            "#,
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(WorkspaceError::UserNotFound(
                "workspace member not found or cannot be removed".to_string(),
            ));
        }

        Ok(())
    }

    async fn member_count(&self, workspace_id: Uuid) -> Result<i64, WorkspaceError> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM TBL_WORKSPACE_MEMBERS
            WHERE workspace_id = $1
              AND is_soft_deleted = FALSE
              AND deleted_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        Ok(count)
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(db_error)
            if db_error.code().as_deref() == Some("23505")
    )
}
