use chrono::{DateTime, Utc};
use shared::{OrgId, UserId, WorkspaceId};
use std::sync::Arc;
use uuid::Uuid;

use super::{
    domain::{MemberRole, Workspace, WorkspaceMember, WorkspaceMemberWithUser, WorkspaceView},
    dto::{
        CreateWorkspaceRequest, InviteMemberRequest, UpdateMemberRoleRequest,
        UpdateWorkspaceRequest,
    },
    error::WorkspaceError,
    repository::WorkspaceRepo,
};
use crate::services::outbox::{events::OutboxEvent, OutboxPublisher};

pub struct WorkspaceUseCases {
    repo: Arc<dyn WorkspaceRepo>,
}

impl WorkspaceUseCases {
    pub fn new(repo: Arc<dyn WorkspaceRepo>) -> Self {
        Self { repo }
    }

    pub async fn list(
        &self,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<Vec<WorkspaceView>, WorkspaceError> {
        self.repo.list_for_user(user_id, org_id).await
    }

    pub async fn get(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<WorkspaceView, WorkspaceError> {
        let role = self.require_member(workspace_id, user_id).await?;

        let workspace = self
            .repo
            .find_by_id(workspace_id)
            .await?
            .ok_or(WorkspaceError::NotFound)?;

        let user_role = role.as_str().to_string();

        let member_count = self.repo.member_count(workspace_id).await?;

        Ok(WorkspaceView {
            id: workspace.id.0,
            organization_id: workspace.organization_id.0,
            name: workspace.name,
            description: workspace.description,
            color: workspace.color,
            created_by: workspace.created_by,
            created_at: workspace.created_at,
            member_count,
            user_role,
            updated_at: workspace.updated_at,
            updated_by: workspace.updated_by,
        })
    }
    // ── Create workspace ──────────────────────────────────────────────────────

    pub async fn create(
        &self,
        req: CreateWorkspaceRequest,
        user_id: Uuid,
        org_id: Uuid,
    ) -> Result<WorkspaceView, WorkspaceError> {
        let workspace = Workspace {
            id: WorkspaceId(uuid::Uuid::new_v4()),
            organization_id: OrgId(org_id),
            name: req.name,
            description: req.description,
            color: req.color,
            created_by: user_id,
            created_at: chrono::Utc::now(),
            updated_at: Some(chrono::Utc::now()),
            deleted_at: None,
            deleted_by: None,
            updated_by: None,
            is_soft_deleted: false,
        };

        let saved = self.repo.create(workspace).await?;

        // Creator automatically becomes owner
        self.repo
            .add_member(saved.id.0, user_id, &MemberRole::Owner, user_id)
            .await?;

        Ok(WorkspaceView {
            id: saved.id.0,
            organization_id: saved.organization_id.0,
            name: saved.name,
            description: saved.description,
            color: saved.color,
            created_by: saved.created_by,
            created_at: saved.created_at,
            member_count: 1,
            user_role: MemberRole::Owner.as_str().to_string(),
            updated_at: saved.updated_at,
            updated_by: saved.updated_by,
        })
    }

    // ── Update workspace ──────────────────────────────────────────────────────

    pub async fn update(
        &self,
        workspace_id: Uuid,
        req: UpdateWorkspaceRequest,
        user_id: Uuid,
    ) -> Result<WorkspaceView, WorkspaceError> {
        let role = self.require_member(workspace_id, user_id).await?;

        // Only owner or admin can update workspace settings
        if !role.can_manage_members() {
            return Err(WorkspaceError::Forbidden);
        }

        let existing = self
            .repo
            .find_by_id(workspace_id)
            .await?
            .ok_or(WorkspaceError::NotFound)?;

        let updated = Workspace {
            name: req.name.unwrap_or(existing.name),
            description: req.description.or(existing.description),
            color: req.color.or(existing.color),
            updated_at: Some(chrono::Utc::now()),
            ..existing
        };

        let saved = self.repo.update(updated).await?;
        let member_count = self.repo.member_count(workspace_id).await?;

        Ok(WorkspaceView {
            id: saved.id.0,
            organization_id: saved.organization_id.0,
            name: saved.name,
            description: saved.description,
            color: saved.color,
            created_by: saved.created_by,
            created_at: saved.created_at,
            member_count,
            user_role: MemberRole::Owner.as_str().to_string(),
            updated_at: saved.updated_at,
            updated_by: saved.updated_by,
        })
    }

    // ── Delete workspace ──────────────────────────────────────────────────────

    pub async fn delete(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), WorkspaceError> {
        let role = self.require_member(workspace_id, user_id).await?;

        // Only owner can delete
        if role != MemberRole::Owner {
            return Err(WorkspaceError::Forbidden);
        }

        self.repo.soft_delete(workspace_id).await
    }

    // ── List members ──────────────────────────────────────────────────────────

    pub async fn list_members(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<WorkspaceMemberWithUser>, WorkspaceError> {
        self.require_member(workspace_id, user_id).await?;
        self.repo.list_members(workspace_id).await
    }

    // ── Invite member ─────────────────────────────────────────────────────────

    pub async fn invite_member(
        &self,
        workspace_id: Uuid,
        req: InviteMemberRequest,
        requester_id: Uuid,
        org_id: Uuid,
    ) -> Result<WorkspaceMemberWithUser, WorkspaceError> {
        let requester_role = self.require_member(workspace_id, requester_id).await?;

        // Only owner or admin can invite
        if !requester_role.can_manage_members() {
            return Err(WorkspaceError::Forbidden);
        }

        // Validate role
        let role = MemberRole::try_from(req.role.as_str())?;

        // Owners cannot be added via invite — only the creator gets that
        if role == MemberRole::Owner {
            return Err(WorkspaceError::Forbidden);
        }

        // Admins cannot invite other admins (owner only)
        if role == MemberRole::Admin && requester_role != MemberRole::Owner {
            return Err(WorkspaceError::Forbidden);
        }

        // Look up user by email within the same org
        let invite_user_id = self
            .repo
            .find_user_by_email(&req.email, org_id)
            .await?
            .ok_or_else(|| WorkspaceError::UserNotFound(req.email.clone()))?;

        // Check not already a member
        if self
            .repo
            .get_member_role(workspace_id, invite_user_id)
            .await?
            .is_some()
        {
            return Err(WorkspaceError::AlreadyMember);
        }

        let mut tx = self.repo.begin().await?;

        self.repo
            .add_member(workspace_id, invite_user_id, &role, requester_id)
            .await?;

        let invitee = self.repo.get_member(requester_id).await?.unwrap();

        let org_workspace_info = self
            .repo
            .get_with_organization(shared::WorkspaceId(workspace_id))
            .await?
            .ok_or_else(|| WorkspaceError::NotFound)?;

        OutboxPublisher::publish_in_tx(
            &OutboxEvent::EmailWorkspaceInvitation {
                to: req.email.clone(),
                display_name: req.email.clone(),
                inviter_name: invitee.user.display_name.clone(),
                org_name: org_workspace_info.name.clone(),
                workspace_name: org_workspace_info.organization.name.clone(),
                role: role.as_str().to_string(),
                workspace_id,
            },
            &mut tx,
        )
        .await
        .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| WorkspaceError::Internal(e.to_string()))?;

        // Return updated member list entry
        let members = self.repo.list_members(workspace_id).await?;

        members
            .into_iter()
            .find(|m| m.member.user_id == UserId(invite_user_id))
            .ok_or_else(|| WorkspaceError::Internal("member not found after insert".into()))
    }

    // ── Update member role ────────────────────────────────────────────────────

    pub async fn update_member_role(
        &self,
        workspace_id: Uuid,
        target_user_id: Uuid,
        req: UpdateMemberRoleRequest,
        requester_id: Uuid,
    ) -> Result<WorkspaceMemberWithUser, WorkspaceError> {
        let requester_role = self.require_member(workspace_id, requester_id).await?;

        if !requester_role.can_manage_members() {
            return Err(WorkspaceError::Forbidden);
        }

        let new_role = MemberRole::try_from(req.role.as_str())?;

        // Cannot promote to owner via this endpoint
        if new_role == MemberRole::Owner {
            return Err(WorkspaceError::Forbidden);
        }

        // Admins cannot change other admins — only owners can
        let target_role = self
            .repo
            .get_member_role(workspace_id, target_user_id)
            .await?
            .ok_or(WorkspaceError::NotFound)?;

        if target_role == MemberRole::Owner {
            return Err(WorkspaceError::CannotRemoveOwner);
        }

        if target_role == MemberRole::Admin && requester_role != MemberRole::Owner {
            return Err(WorkspaceError::Forbidden);
        }

        self.repo
            .update_member_role(workspace_id, target_user_id, &new_role)
            .await?;

        let members = self.repo.list_members(workspace_id).await?;

        members
            .into_iter()
            .find(|m| m.user.id == UserId(target_user_id))
            .ok_or_else(|| WorkspaceError::Internal("member not found after update".into()))
    }

    // ── Remove member ─────────────────────────────────────────────────────────

    pub async fn remove_member(
        &self,
        workspace_id: Uuid,
        target_user_id: Uuid,
        requester_id: Uuid,
    ) -> Result<(), WorkspaceError> {
        let requester_role = self.require_member(workspace_id, requester_id).await?;

        // Members can remove themselves
        let is_self = target_user_id == requester_id;

        if !is_self && !requester_role.can_manage_members() {
            return Err(WorkspaceError::Forbidden);
        }

        // Cannot remove the owner
        let target_role = self
            .repo
            .get_member_role(workspace_id, target_user_id)
            .await?
            .ok_or(WorkspaceError::NotFound)?;

        if target_role == MemberRole::Owner {
            return Err(WorkspaceError::CannotRemoveOwner);
        }

        // Admins cannot remove other admins
        if target_role == MemberRole::Admin && requester_role != MemberRole::Owner && !is_self {
            return Err(WorkspaceError::Forbidden);
        }

        self.repo.remove_member(workspace_id, target_user_id).await
    }

    async fn require_member(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> Result<MemberRole, WorkspaceError> {
        self.repo
            .get_member_role(workspace_id, user_id)
            .await?
            .ok_or(WorkspaceError::Forbidden)
    }
}
