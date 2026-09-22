use rand::RngExt;
use std::sync::Arc;
use uuid::Uuid;

use shared::{OrgId, UserId};

use super::domain::*;
use super::dto::*;
use super::error::OrgError;
use super::repository::OrgRepo;

pub struct OrgUseCases {
    repo: Arc<dyn OrgRepo>,
}

impl OrgUseCases {
    pub fn new(repo: Arc<dyn OrgRepo>) -> Self {
        Self { repo }
    }

    pub async fn get(&self, org_id: Uuid, user_id: Uuid) -> Result<Organization, OrgError> {
        self.require_member(org_id, user_id).await?;
        self.repo
            .find_by_id(org_id)
            .await?
            .ok_or(OrgError::NotFound)
    }

    // ── Create organization ───────────────────────────────────────────────────

    pub async fn create(
        &self,
        req: CreateOrgRequest,
        user_id: Uuid,
    ) -> Result<Organization, OrgError> {
        let slug = req.slug.unwrap_or_else(|| slugify(&req.name));

        // Check slug availability
        if self.repo.find_by_slug(&slug).await?.is_some() {
            return Err(OrgError::SlugTaken);
        }

        let org = Organization {
            id: OrgId(Uuid::new_v4()),
            name: req.name,
            slug,
            plan: super::domain::OrgPlan::Free,
            logo_url: None,
            website: req.website,
            sso_enabled: false,
            max_workspaces: super::domain::OrgPlan::Free.max_workspaces(),
            max_members: super::domain::OrgPlan::Free.max_members(),
            max_connections: super::domain::OrgPlan::Free.max_connections(),
            ai_credits_limit: super::domain::OrgPlan::Free.ai_credits_limit(),
            ai_credits_used: 0,
            trial_ends_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let saved = self.repo.create(org, user_id).await?;

        // Creator becomes owner
        self.repo
            .add_member(saved.id.0, user_id, &OrgRole::Owner, None)
            .await?;

        Ok(saved)
    }

    // ── Update organization settings ──────────────────────────────────────────

    pub async fn update(
        &self,
        org_id: Uuid,
        req: UpdateOrgRequest,
        user_id: Uuid,
    ) -> Result<Organization, OrgError> {
        let role = self.require_member(org_id, user_id).await?;

        if !role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        let existing = self
            .repo
            .find_by_id(org_id)
            .await?
            .ok_or(OrgError::NotFound)?;

        let updated = Organization {
            name: req.name.unwrap_or(existing.name),
            logo_url: req.logo_url.or(existing.logo_url),
            website: req.website.or(existing.website),
            updated_at: chrono::Utc::now(),
            ..existing
        };

        let saved = self.repo.update(updated).await?;

        // self.audit(AuditEvent::settings_updated(org_id, user_id))
        //     .await;

        Ok(saved)
    }

    // ── Usage ─────────────────────────────────────────────────────────────────

    pub async fn get_usage(&self, org_id: Uuid, user_id: Uuid) -> Result<OrgUsageView, OrgError> {
        self.require_member(org_id, user_id).await?;
        self.repo.get_usage(org_id).await
    }

    // ── Members ───────────────────────────────────────────────────────────────

    pub async fn list_members(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<OrgMember>, OrgError> {
        self.require_member(org_id, user_id).await?;
        self.repo.list_members(org_id).await
    }

    pub async fn update_member_role(
        &self,
        org_id: Uuid,
        target_user_id: Uuid,
        req: UpdateMemberRoleRequest,
        requester_id: Uuid,
    ) -> Result<OrgMember, OrgError> {
        let requester_role = self.require_member(org_id, requester_id).await?;

        if !requester_role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        let new_role = OrgRole::try_from(req.role.as_str())?;

        // Cannot promote to owner
        if new_role == OrgRole::Owner {
            return Err(OrgError::Forbidden);
        }

        // Verify target is a member
        let target_role = self
            .repo
            .get_member_role(org_id, target_user_id)
            .await?
            .ok_or(OrgError::NotFound)?;

        // Cannot demote the owner
        if target_role == OrgRole::Owner {
            return Err(OrgError::CannotRemoveOwner);
        }

        // Admins cannot change other admins
        if target_role == OrgRole::Admin && requester_role != OrgRole::Owner {
            return Err(OrgError::Forbidden);
        }

        self.repo
            .update_member_role(org_id, target_user_id, &new_role)
            .await?;

        // self.audit(AuditEvent::role_changed(
        //     org_id,
        //     requester_id,
        //     target_user_id,
        //     new_role.as_str(),
        // ))
        // .await;

        let members = self.repo.list_members(org_id).await?;
        members
            .into_iter()
            .find(|m| m.user_id == UserId(target_user_id))
            .ok_or_else(|| OrgError::Internal("member not found after update".into()))
    }

    pub async fn remove_member(
        &self,
        org_id: Uuid,
        target_user_id: Uuid,
        requester_id: Uuid,
    ) -> Result<(), OrgError> {
        let requester_role = self.require_member(org_id, requester_id).await?;

        let is_self = target_user_id == requester_id;

        if !is_self && !requester_role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        let target_role = self
            .repo
            .get_member_role(org_id, target_user_id)
            .await?
            .ok_or(OrgError::NotFound)?;

        if target_role == OrgRole::Owner {
            return Err(OrgError::CannotRemoveOwner);
        }

        if target_role == OrgRole::Admin && requester_role != OrgRole::Owner && !is_self {
            return Err(OrgError::Forbidden);
        }

        self.repo.remove_member(org_id, target_user_id).await?;

        // self.audit(AuditEvent::member_removed(
        //     org_id,
        //     requester_id,
        //     target_user_id,
        // ))
        // .await;

        Ok(())
    }

    // ── Invitations ───────────────────────────────────────────────────────────

    pub async fn invite_member(
        &self,
        org_id: Uuid,
        req: InviteMemberRequest,
        requester_id: Uuid,
    ) -> Result<OrgInvitation, OrgError> {
        let requester_role = self.require_member(org_id, requester_id).await?;

        if !requester_role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        let role = OrgRole::try_from(req.role.as_str())?;

        if role == OrgRole::Owner {
            return Err(OrgError::Forbidden);
        }

        if role == OrgRole::Admin && requester_role != OrgRole::Owner {
            return Err(OrgError::Forbidden);
        }

        // Check plan limit
        let usage = self.repo.get_usage(org_id).await?;
        if usage.member_count >= usage.limits.max_members as i64 {
            return Err(OrgError::PlanLimitReached(format!(
                "Member limit of {} reached. Upgrade your plan to add more members.",
                usage.limits.max_members
            )));
        }

        // Generate secure invitation token
        let token = generate_invite_token();

        let invitation = self
            .repo
            .create_invitation(org_id, &req.email, &role, requester_id, &token)
            .await?;

        // self.audit(AuditEvent::member_invited(
        //     org_id,
        //     requester_id,
        //     &req.email,
        //     role.as_str(),
        // ))
        // .await;

        // TODO: send invitation email with token link
        // email_service.send_invitation(&invitation).await?;

        Ok(invitation)
    }

    pub async fn list_pending_invitations(
        &self,
        org_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<OrgInvitation>, OrgError> {
        let role = self.require_member(org_id, user_id).await?;

        if !role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        self.repo.list_pending_invitations(org_id).await
    }

    pub async fn revoke_invitation(
        &self,
        org_id: Uuid,
        invitation_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), OrgError> {
        let role = self.require_member(org_id, user_id).await?;

        if !role.can_manage_members() {
            return Err(OrgError::Forbidden);
        }

        self.repo.revoke_invitation(invitation_id).await
    }

    pub async fn accept_invitation(
        &self,
        req: AcceptInvitationRequest,
        user_id: Uuid,
    ) -> Result<Organization, OrgError> {
        let invitation = self
            .repo
            .find_invitation_by_token(&req.token)
            .await?
            .ok_or(OrgError::InvitationNotFound)?;

        if invitation.accepted_at.is_some() {
            return Err(OrgError::InvitationAlreadyAccepted);
        }

        if chrono::Utc::now() > invitation.expires_at {
            return Err(OrgError::InvitationNotFound);
        }

        // Check if already a member
        if self
            .repo
            .get_member_role(invitation.org_id.0, user_id)
            .await?
            .is_some()
        {
            return Err(OrgError::AlreadyMember);
        }

        // Accept — marks invitation as accepted + adds member
        self.repo.accept_invitation(&req.token, user_id).await?;

        // self.audit(AuditEvent::member_joined(invitation.org_id, user_id))
        //     .await;

        self.repo
            .find_by_id(invitation.org_id.0)
            .await?
            .ok_or(OrgError::NotFound)
    }

    // ── Audit log ─────────────────────────────────────────────────────────────

    // pub async fn list_audit_logs(
    //     &self,
    //     org_id: Uuid,
    //     user_id: Uuid,
    //     query: AuditLogQuery,
    // ) -> Result<Vec<OrgAuditLog>, OrgError> {
    //     let role = self.require_member(org_id, user_id).await?;

    //     if !role.can_view_audit_log() {
    //         return Err(OrgError::Forbidden);
    //     }

    //     self.repo.list_audit_logs(org_id, &query).await
    // }

    // ── AI credits ────────────────────────────────────────────────────────────

    pub async fn consume_ai_credits(&self, org_id: Uuid, amount: i32) -> Result<(), OrgError> {
        let org = self
            .repo
            .find_by_id(org_id)
            .await?
            .ok_or(OrgError::NotFound)?;

        if org.ai_credits_used + amount > org.ai_credits_limit {
            return Err(OrgError::PlanLimitReached(format!(
                "AI credit limit of {} reached. Upgrade your plan for more.",
                org.ai_credits_limit
            )));
        }

        self.repo.increment_ai_credits(org_id, amount).await
    }

    async fn require_member(&self, org_id: Uuid, user_id: Uuid) -> Result<OrgRole, OrgError> {
        self.repo
            .get_member_role(org_id, user_id)
            .await?
            .ok_or(OrgError::Forbidden)
    }

    // async fn audit(&self, event: AuditEvent) {
    //     if let Err(e) = self.repo.write_audit_log(event).await {
    //         tracing::warn!(error = %e, "audit log write failed");
    //     }
    // }
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

fn generate_invite_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    hex::encode(bytes)
}
