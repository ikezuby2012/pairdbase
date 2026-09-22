use crate::{
    abstractions::ApiResponse,
    features::{
        connections::drivers::trait_::DatabaseMetadata, organization::domain::OrganizationView,
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::domain::{
    OrgInvitation, OrgInvitationView, OrgMemberWithUserView, OrgUsageView, Organization,
};
use super::dto::*;
use super::error::OrgError;

use crate::{extractors::AuthUser, state::AppState};

pub async fn create(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateOrgRequest>,
) -> Result<ApiResponse<OrganizationView>, OrgError> {
    let org = state.organization.create(body, auth.user_id).await?;

    Ok(ApiResponse::success(
        OrganizationView::from(org),
        "Organization was created successfully",
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
) -> Result<ApiResponse<OrganizationView>, OrgError> {
    let org = state.organization.get(org_id, auth.user_id).await?;

    Ok(ApiResponse::success(
        OrganizationView::from(org),
        "Organization retrieved successfully",
    ))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
    Json(body): Json<UpdateOrgRequest>,
) -> Result<ApiResponse<OrganizationView>, OrgError> {
    let org = state
        .organization
        .update(org_id, body, auth.user_id)
        .await?;

    Ok(ApiResponse::success(
        OrganizationView::from(org),
        "Organization updated successfully",
    ))
}

pub async fn get_usage(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
) -> Result<ApiResponse<OrgUsageView>, OrgError> {
    let usage = state.organization.get_usage(org_id, auth.user_id).await?;

    Ok(ApiResponse::success(
        OrgUsageView::from(usage),
        "Organization usage retrieved successfully",
    ))
}

pub async fn list_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<OrgMemberWithUserView>>, OrgError> {
    let members = state
        .organization
        .list_members(org_id, auth.user_id)
        .await?;

    let members = members
        .into_iter()
        .map(OrgMemberWithUserView::from)
        .collect();

    Ok(ApiResponse::success(
        members,
        "Organization members retrieved successfully",
    ))
}

pub async fn update_member_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((org_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMemberRoleRequest>,
) -> Result<ApiResponse<OrgMemberWithUserView>, OrgError> {
    let member = state
        .organization
        .update_member_role(org_id, target_user_id, body, auth.user_id)
        .await?;

    Ok(ApiResponse::success(
        OrgMemberWithUserView::from(member),
        "Member role updated successfully",
    ))
}

pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((org_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, OrgError> {
    state
        .organization
        .remove_member(org_id, target_user_id, auth.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn invite_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
    Json(body): Json<InviteMemberRequest>,
) -> Result<ApiResponse<OrgInvitationView>, OrgError> {
    let invitation = state
        .organization
        .invite_member(org_id, body, auth.user_id)
        .await?;

    Ok(ApiResponse::success(
        OrgInvitationView::from(invitation),
        "Invitation sent successfully",
    ))
}

pub async fn list_invitations(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(org_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<OrgInvitation>>, OrgError> {
    let invitations = state
        .organization
        .list_pending_invitations(org_id, auth.user_id)
        .await?;
    Ok(ApiResponse::success(invitations, "List all invitations"))
}

pub async fn revoke_invitation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((org_id, invitation_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, OrgError> {
    state
        .organization
        .revoke_invitation(org_id, invitation_id, auth.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn accept_invitation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(token): Path<String>,
) -> Result<ApiResponse<OrganizationView>, OrgError> {
    let org = state
        .organization
        .accept_invitation(AcceptInvitationRequest { token }, auth.user_id)
        .await?;

    Ok(ApiResponse::success(
        OrganizationView::from(org),
        "Invitation accepted successfully",
    ))
}

// pub async fn list_audit_logs(
//     State(state): State<Arc<AppState>>,
//     auth: AuthUser,
//     Path(org_id): Path<Uuid>,
//     Query(query): Query<AuditLogQuery>,
// ) -> Result<ApiResponse<Vec<OrgAuditLog>>, OrgError> {
//     let logs = state
//         .organizations
//         .list_audit_logs(org_id, auth.user_id, query)
//         .await?;
//     Ok(ApiResponse::success(logs, ""))
// }
