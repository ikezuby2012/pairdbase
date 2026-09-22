use aide::{
    axum::{
        routing::{delete_with, get_with, post_with, put_with},
        ApiRouter,
    },
    transform::TransformOperation,
};

use axum::Json;

use crate::abstractions::responses::{ApiErrorResponse, ApiResponseSchema};

use crate::state::SharedState;

pub mod domain;
pub mod dto;
pub mod error;
pub mod handler;
pub mod repository;
pub mod use_case;

use domain::{OrgInvitationView, OrgMemberView, OrgUsageView, Organization};
use handler::*;

pub fn router() -> ApiRouter<SharedState> {
    ApiRouter::new()
        // Organization
        .api_route("/", post_with(create, create_docs))
        .api_route(
            "/{org_id}",
            get_with(get, get_docs).put_with(update, update_docs),
        )
        // Usage
        .api_route("/{org_id}/usage", get_with(get_usage, get_usage_docs))
        // Members
        .api_route(
            "/{org_id}/members",
            get_with(list_members, list_members_docs),
        )
        .api_route(
            "/{org_id}/members/{user_id}",
            put_with(update_member_role, update_member_role_docs)
                .delete_with(remove_member, remove_member_docs),
        )
        // Invitations
        .api_route(
            "/{org_id}/invitations",
            get_with(list_invitations, list_invitations_docs)
                .post_with(invite_member, invite_member_docs),
        )
        .api_route(
            "/{org_id}/invitations/{invitation_id}",
            delete_with(revoke_invitation, revoke_invitation_docs),
        )
        // Accept invitation (no org_id — user may not be a member yet)
        .api_route(
            "/invitations/accept",
            post_with(accept_invitation, accept_invitation_docs),
        )
    // Audit log
    // .api_route("/:org_id/audit",
    //     get_with(list_audit_logs, list_audit_docs)
    //)
}

// ── Docs ──────────────────────────────────────────────────────────────────────

fn create_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create organization")
        .description("Creates a new organization. Creator becomes the Owner.")
        .tag("organizations")
        .response::<201, Json<ApiResponseSchema<Organization>>>()
        .response_with::<409, Json<serde_json::Value>, _>(|r| r.description("Slug already taken"))
        .security_requirement("bearer_auth")
}

fn get_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get organization")
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<Organization>>>()
        .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Not a member"))
        .security_requirement("bearer_auth")
}

fn update_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update organization settings")
        .description("Requires Owner or Admin role.")
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<Organization>>>()
        .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Forbidden"))
        .security_requirement("bearer_auth")
}

fn get_usage_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get usage stats")
        .description("Returns workspace, member, connection counts and AI credit usage.")
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<OrgUsageView>>>()
        .security_requirement("bearer_auth")
}

fn list_members_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List organization members")
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<Vec<OrgMemberView>>>>()
        .security_requirement("bearer_auth")
}

fn update_member_role_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update member role")
        .description(
            "Change a member's org role. \
           Cannot promote to Owner. \
           Admins cannot change other Admins — Owner only.",
        )
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<OrgMemberView>>>()
        .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Forbidden"))
        .security_requirement("bearer_auth")
}

fn remove_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove a member")
        .description("Members can remove themselves. Owner cannot be removed.")
        .tag("organizations")
        .response_with::<204, Json<serde_json::Value>, _>(|r| r.description("Removed"))
        .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Forbidden"))
        .security_requirement("bearer_auth")
}

fn invite_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Invite a member")
        .description(
            "Sends an invitation to an email address. \
           User does not need to be registered yet. \
           Token expires in 7 days. \
           Re-inviting the same email resets the token and expiry.",
        )
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<OrgInvitationView>>>()
        .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Forbidden"))
        .response_with::<429, Json<serde_json::Value>, _>(|r| {
            r.description("Plan member limit reached")
        })
        .security_requirement("bearer_auth")
}

fn list_invitations_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List pending invitations")
        .description("Returns all unaccepted, non-expired invitations. Requires Owner or Admin.")
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<Vec<OrgInvitationView>>>>()
        .security_requirement("bearer_auth")
}

fn revoke_invitation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Revoke an invitation")
        .description("Cancels a pending invitation. Requires Owner or Admin.")
        .tag("organizations")
        .response_with::<204, Json<serde_json::Value>, _>(|r| r.description("revoked"))
        .security_requirement("bearer_auth")
}

fn accept_invitation_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Accept an invitation")
        .description(
            "Accept an organization invitation using the token from the email link. \
           User must be authenticated. \
           Returns the organization the user just joined.",
        )
        .tag("organizations")
        .response::<200, Json<ApiResponseSchema<Organization>>>()
        .response_with::<404, Json<serde_json::Value>, _>(|r| {
            r.description("Token not found or expired")
        })
        .response_with::<409, Json<serde_json::Value>, _>(|r| {
            r.description("Already a member or invitation already accepted")
        })
        .security_requirement("bearer_auth")
}

// fn list_audit_docs(op: TransformOperation) -> TransformOperation {
//     op.summary("List audit logs")
//       .description(
//           "Returns organization audit log entries. \
//            Requires Owner or Admin role. \
//            Supports filtering by actor_id and event_type."
//       )
//       .tag("organizations")
//       .response::<200, Json<ApiResponseSchema<Vec<OrgAuditLog>>>>()
//       .response_with::<403, Json<serde_json::Value>, _>(|r| r.description("Forbidden"))
//       .security_requirement("bearer_auth")
// }
