use async_trait::async_trait;
use uuid::Uuid;

use crate::db::DbPool;

use super::domain::{
    OrgInvitation, OrgInvitationRow, OrgLimitsView, OrgMember, OrgMemberWithUserView, OrgRole,
    OrgUsageView, Organization, OrganizationRow,
};
use super::error::OrgError;

#[async_trait]
pub trait OrgRepo: Send + Sync {
    // Organization
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrgError>;
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, OrgError>;
    async fn create(&self, org: Organization, created_by: Uuid) -> Result<Organization, OrgError>;
    async fn update(&self, org: Organization) -> Result<Organization, OrgError>;

    // Members
    async fn get_member_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrgRole>, OrgError>;

    async fn list_members(&self, org_id: Uuid) -> Result<Vec<OrgMember>, OrgError>;

    async fn add_member(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &OrgRole,
        invited_by: Option<Uuid>,
    ) -> Result<(), OrgError>;

    async fn update_member_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &OrgRole,
    ) -> Result<(), OrgError>;

    async fn remove_member(&self, org_id: Uuid, user_id: Uuid) -> Result<(), OrgError>;

    // Invitations
    async fn create_invitation(
        &self,
        org_id: Uuid,
        email: &str,
        role: &OrgRole,
        invited_by: Uuid,
        token: &str,
    ) -> Result<OrgInvitation, OrgError>;

    async fn find_invitation_by_token(
        &self,
        token: &str,
    ) -> Result<Option<OrgInvitation>, OrgError>;

    async fn accept_invitation(
        &self,
        token: &str,
        user_id: Uuid,
    ) -> Result<OrgInvitation, OrgError>;

    async fn list_pending_invitations(&self, org_id: Uuid) -> Result<Vec<OrgInvitation>, OrgError>;

    async fn revoke_invitation(&self, id: Uuid) -> Result<(), OrgError>;

    // Usage
    async fn get_usage(&self, org_id: Uuid) -> Result<OrgUsageView, OrgError>;
    async fn increment_ai_credits(&self, org_id: Uuid, amount: i32) -> Result<(), OrgError>;

    // Audit
    // async fn write_audit_log(&self, event: AuditEvent) -> Result<(), OrgError>;
    // async fn list_audit_logs(
    //     &self, org_id: Uuid, query: &AuditLogQuery,
    // ) -> Result<Vec<OrgAuditLog>, OrgError>;
}

pub struct PgOrganizationRepo {
    pool: DbPool,
}

impl PgOrganizationRepo {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OrgRepo for PgOrganizationRepo {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Organization>, OrgError> {
        let row = sqlx::query_as::<_, OrganizationRow>(
            r#"
            SELECT
                id,
                name,
                slug,
                plan,
                logo_url,
                website,
                sso_enabled,
                max_workspaces,
                max_members,
                max_connections,
                ai_credits_limit,
                ai_credits_used,
                trial_ends_at,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_ORGANIZATIONS
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        row.map(Organization::try_from).transpose()
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Organization>, OrgError> {
        let row = sqlx::query_as::<_, OrganizationRow>(
            r#"
            SELECT
                id,
                name,
                slug,
                plan,
                logo_url,
                website,
                sso_enabled,
                max_workspaces,
                max_members,
                max_connections,
                ai_credits_limit,
                ai_credits_used,
                trial_ends_at,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_ORGANIZATIONS
            WHERE LOWER(slug) = LOWER($1)
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        row.map(Organization::try_from).transpose()
    }

    async fn create(&self, org: Organization, created_by: Uuid) -> Result<Organization, OrgError> {
        let row = sqlx::query_as::<_, OrganizationRow>(
            r#"
        INSERT INTO TBL_ORGANIZATIONS (
            id,
            name,
            slug,
            plan,
            logo_url,
            website,
            sso_enabled,
            max_workspaces,
            max_members,
            max_connections,
            ai_credits_limit,
            ai_credits_used,
            trial_ends_at,
            created_by
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
            $10,
            $11,
            $12,
            $13,
            $14
        )
        RETURNING
            id,
            name,
            slug,
            plan,
            logo_url,
            website,
            sso_enabled,
            max_workspaces,
            max_members,
            max_connections,
            ai_credits_limit,
            ai_credits_used,
            trial_ends_at,
            created_by,
            created_at,
            updated_by,
            updated_at,
            is_soft_deleted,
            deleted_by,
            deleted_at
        "#,
        )
        .bind(org.id.0)
        .bind(&org.name)
        .bind(&org.slug)
        .bind(org.plan.as_str())
        .bind(&org.logo_url)
        .bind(&org.website)
        .bind(org.sso_enabled)
        .bind(org.max_workspaces)
        .bind(org.max_members)
        .bind(org.max_connections)
        .bind(org.ai_credits_limit)
        .bind(org.ai_credits_used)
        .bind(org.trial_ends_at)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        Organization::try_from(row)
    }

    async fn update(&self, org: Organization) -> Result<Organization, OrgError> {
        let row = sqlx::query_as::<_, OrganizationRow>(
            r#"
            UPDATE TBL_ORGANIZATIONS
            SET
                name = $2,
                slug = $3,
                plan = $4,
                logo_url = $5,
                website = $6,
                sso_enabled = $7,
                max_workspaces = $8,
                max_members = $9,
                max_connections = $10,
                ai_credits_limit = $11,
                ai_credits_used = $12,
                trial_ends_at = $13,
                updated_at = $14
            WHERE id = $1
            RETURNING
                id,
                name,
                slug,
                plan,
                logo_url,
                website,
                sso_enabled,
                max_workspaces,
                max_members,
                max_connections,
                ai_credits_limit,
                ai_credits_used,
                trial_ends_at,
                created_at,
                updated_at
            "#,
        )
        .bind(org.id.0)
        .bind(&org.name)
        .bind(&org.slug)
        .bind(org.plan.as_str())
        .bind(&org.logo_url)
        .bind(&org.website)
        .bind(org.sso_enabled)
        .bind(org.max_workspaces)
        .bind(org.max_members)
        .bind(org.max_connections)
        .bind(org.ai_credits_limit)
        .bind(org.ai_credits_used)
        .bind(org.trial_ends_at)
        .bind(org.updated_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?
        .ok_or(OrgError::NotFound)?;

        Organization::try_from(row)
    }

    async fn get_member_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<OrgRole>, OrgError> {
        let role: Option<String> = sqlx::query_scalar(
            r#"
            SELECT role
            FROM TBL_ORGANIZATION_MEMBERS
            WHERE org_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        role.map(|role| {
            OrgRole::try_from(role.as_str()).map_err(|e| OrgError::Internal(e.to_string()))
        })
        .transpose()
    }

    async fn list_members(&self, org_id: Uuid) -> Result<Vec<OrgMember>, OrgError> {
        let rows = sqlx::query_as::<_, OrgMemberWithUserView>(
            r#"
            SELECT
                om.id,
                om.org_id,
                om.user_id,
                om.role,
                om.joined_at,
                u.email,
                u.display_name,
                u.avatar_url,
                om.created_by,
                om.created_at,
                om.updated_by,
                om.updated_at
            FROM TBL_ORGANIZATION_MEMBERS om
            INNER JOIN TBL_USERS u
                ON u.id = om.user_id
            WHERE om.org_id = $1
              AND om.is_soft_deleted = FALSE
            ORDER BY om.joined_at ASC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        rows.into_iter().map(OrgMember::try_from).collect()
    }

    async fn add_member(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &OrgRole,
        invited_by: Option<Uuid>,
    ) -> Result<(), OrgError> {
        sqlx::query(
            r#"
            INSERT INTO TBL_ORGANIZATION_MEMBERS (
                org_id,
                user_id,
                role,
                joined_at,
                created_by
            )
            VALUES ($1, $2, $3, NOW(), $4)
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role.as_str())
        .bind(invited_by)
        .execute(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn update_member_role(
        &self,
        org_id: Uuid,
        user_id: Uuid,
        role: &OrgRole,
    ) -> Result<(), OrgError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_ORGANIZATION_MEMBERS
            SET
                role = $3,
                updated_at = NOW()
            WHERE org_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrgError::NotFound);
        }

        Ok(())
    }

    async fn remove_member(&self, org_id: Uuid, user_id: Uuid) -> Result<(), OrgError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_ORGANIZATION_MEMBERS
            SET
                is_soft_deleted = TRUE,
                deleted_at = NOW()
            WHERE org_id = $1
              AND user_id = $2
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrgError::NotFound);
        }

        Ok(())
    }

    async fn create_invitation(
        &self,
        org_id: Uuid,
        email: &str,
        role: &OrgRole,
        invited_by: Uuid,
        token: &str,
    ) -> Result<OrgInvitation, OrgError> {
        let row = sqlx::query_as::<_, OrgInvitationRow>(
            r#"
            INSERT INTO TBL_ORGANIZATION_INVITATIONS (
                org_id,
                email,
                role,
                invited_by,
                token,
                expires_at,
                created_at
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                NOW() + INTERVAL '7 days',
                NOW()
            )
            RETURNING
                id,
                org_id,
                email,
                role,
                invited_by,
                expires_at,
                created_by,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            "#,
        )
        .bind(org_id)
        .bind(email)
        .bind(role.as_str())
        .bind(invited_by)
        .bind(token)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        OrgInvitation::try_from(row)
    }

    async fn find_invitation_by_token(
        &self,
        token: &str,
    ) -> Result<Option<OrgInvitation>, OrgError> {
        let row = sqlx::query_as::<_, OrgInvitationRow>(
            r#"
            SELECT
                id,
                org_id,
                email,
                role,
                invited_by,
                accepted_by,
                accepted_at,
                expires_at,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_ORGANIZATION_INVITATIONS
            WHERE token = $1
              AND accepted_at IS NULL
              AND revoked_at IS NULL
              AND expires_at > NOW()
            "#,
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        row.map(OrgInvitation::try_from).transpose()
    }

    async fn accept_invitation(
        &self,
        token: &str,
        user_id: Uuid,
    ) -> Result<OrgInvitation, OrgError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| OrgError::Internal(e.to_string()))?;

        let invitation = sqlx::query_as::<_, OrgInvitationRow>(
            r#"
            SELECT
                id,
                org_id,
                email,
                role,
                invited_by,
                expires_at,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_ORGANIZATION_INVITATIONS
            WHERE token = $1
              AND accepted_at IS NULL
              AND revoked_at IS NULL
              AND expires_at > NOW()
            FOR UPDATE
            "#,
        )
        .bind(token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?
        .ok_or(OrgError::InvitationNotFound)?;

        sqlx::query(
            r#"
            INSERT INTO TBL_ORGANIZATION_MEMBERS (
                org_id,
                user_id,
                role,
                joined_at,
                created_by
            )
            VALUES ($1, $2, $3, NOW(), $2)
            "#,
        )
        .bind(invitation.org_id)
        .bind(user_id)
        .bind(&invitation.role)
        .execute(&mut *tx)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        sqlx::query(
            r#"
            UPDATE TBL_ORGANIZATION_INVITATIONS
            SET
                accepted_at = NOW(),
                accepted_by = $2
            WHERE id = $1
            "#,
        )
        .bind(invitation.id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| OrgError::Internal(e.to_string()))?;

        OrgInvitation::try_from(invitation)
    }

    async fn list_pending_invitations(&self, org_id: Uuid) -> Result<Vec<OrgInvitation>, OrgError> {
        let rows = sqlx::query_as::<_, OrgInvitationRow>(
            r#"
            SELECT
                id,
                org_id,
                email,
                role,
                invited_by,
                expires_at,
                created_at,
                updated_by,
                updated_at,
                is_soft_deleted,
                deleted_by,
                deleted_at
            FROM TBL_ORGANIZATION_INVITATIONS
            WHERE org_id = $1
              AND accepted_at IS NULL
              AND revoked_at IS NULL
              AND expires_at > NOW()
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        rows.into_iter().map(OrgInvitation::try_from).collect()
    }

    async fn revoke_invitation(&self, id: Uuid) -> Result<(), OrgError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_ORGANIZATION_INVITATIONS
            SET revoked_at = NOW()
            WHERE id = $1
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrgError::InvitationNotFound);
        }

        Ok(())
    }

    async fn get_usage(&self, org_id: Uuid) -> Result<OrgUsageView, OrgError> {
        let org = sqlx::query_as::<_, OrganizationRow>(
            r#"
            SELECT
                id,
                name,
                slug,
                plan,
                logo_url,
                website,
                sso_enabled,
                max_workspaces,
                max_members,
                max_connections,
                ai_credits_limit,
                ai_credits_used,
                trial_ends_at,
                created_at,
                updated_at
            FROM TBL_ORGANIZATIONS
            WHERE id = $1
            "#,
        )
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?
        .ok_or(OrgError::NotFound)?;

        let workspace_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM TBL_WORKSPACES
            WHERE organization_id = $1
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        let member_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM TBL_ORGANIZATION_MEMBERS
            WHERE org_id = $1
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        let connection_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM TBL_CONNECTIONS
            WHERE organization_id = $1
              AND is_soft_deleted = FALSE
            "#,
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        let org = Organization::try_from(org)?;

        Ok(OrgUsageView {
            workspace_count,
            member_count,
            connection_count,
            ai_credits_used: org.ai_credits_used,
            ai_credits_limit: org.ai_credits_limit,
            limits: OrgLimitsView {
                max_workspaces: org.max_workspaces,
                max_members: org.max_members,
                max_connections: org.max_connections,
            },
        })
    }

    async fn increment_ai_credits(&self, org_id: Uuid, amount: i32) -> Result<(), OrgError> {
        let result = sqlx::query(
            r#"
            UPDATE TBL_ORGANIZATIONS
            SET ai_credits_used = ai_credits_used + $2,
                updated_at = NOW()
            WHERE id = $1
              AND ai_credits_used + $2 <= ai_credits_limit
            "#,
        )
        .bind(org_id)
        .bind(amount)
        .execute(&self.pool)
        .await
        .map_err(|e| OrgError::Internal(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(OrgError::PlanLimitReached(
                "AI credit limit reached".to_string(),
            ));
        }

        Ok(())
    }
}
