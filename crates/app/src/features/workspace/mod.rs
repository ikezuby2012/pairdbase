use aide::{
    axum::{
        routing::{delete_with, get_with, post_with, put_with},
        ApiRouter,
    },
    transform::TransformOperation,
};

use axum::Json;

use crate::abstractions::responses::{ApiErrorResponse, ApiResponseSchema};

pub mod domain;
pub mod dto;
pub mod error;
pub mod handler;
pub mod repository;
pub mod use_case;

use crate::state::SharedState;

use domain::{WorkspaceMemberWithUser, WorkspaceView};
use handler::*;

pub fn router() -> ApiRouter<SharedState> {
    ApiRouter::new()
        .api_route(
            "/",
            get_with(list, list_docs).post_with(create, create_docs),
        )
        .api_route(
            "/{workspace_id}",
            get_with(get, get_docs)
                .put_with(update, update_docs)
                .delete_with(delete, delete_docs),
        )
        .api_route(
            "/{workspace_id}/members",
            get_with(list_members, list_members_docs).post_with(invite_member, invite_member_docs),
        )
        .api_route(
            "/{workspace_id}/members/{user_id}",
            put_with(update_member_role, update_member_role_docs)
                .delete_with(remove_member, remove_member_docs),
        )
}

// ── Docs ──────────────────────────────────────────────────────────────────────

fn list_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspaces")
        .description("Returns all workspaces the requesting user is a member of.")
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<Vec<WorkspaceView>>>>()
        .response_with::<401, Json<ApiErrorResponse>, _>(|r| r.description("Unauthorized"))
        .security_requirement("bearer_auth")
}

fn create_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Create workspace")
        .description("Creates a new workspace. Creator is automatically assigned the Owner role.")
        .tag("workspaces")
        .response::<201, Json<ApiResponseSchema<WorkspaceView>>>()
        .response_with::<409, Json<ApiErrorResponse>, _>(|r| r.description("Name already exists"))
        .security_requirement("bearer_auth")
}

fn get_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Get workspace")
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<WorkspaceView>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| r.description("Not a member"))
        .response_with::<404, Json<ApiErrorResponse>, _>(|r| r.description("Not found"))
        .security_requirement("bearer_auth")
}

fn update_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update workspace")
        .description(
            "Partial update — only provided fields are changed. Requires Owner or Admin role.",
        )
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<WorkspaceView>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| {
            r.description("Forbidden — requires Owner or Admin")
        })
        .security_requirement("bearer_auth")
}

fn delete_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Delete workspace")
        .description("Soft-deletes the workspace. Requires Owner role.")
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<bool>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| {
            r.description("Forbidden — requires Owner")
        })
        .security_requirement("bearer_auth")
}

fn list_members_docs(op: TransformOperation) -> TransformOperation {
    op.summary("List workspace members")
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<Vec<WorkspaceMemberWithUser>>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| r.description("Not a member"))
        .security_requirement("bearer_auth")
}

fn invite_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Invite a member")
        .description(
            "Invite a user by email. User must belong to the same organization. \
           Requires Owner or Admin role. \
           Admins cannot invite other Admins — only Owners can.",
        )
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<WorkspaceMemberWithUser>>>()
        .response_with::<400, Json<ApiErrorResponse>, _>(|r| r.description("Invalid role"))
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| r.description("Forbidden"))
        .response_with::<404, Json<ApiErrorResponse>, _>(|r| {
            r.description("User not found in organization")
        })
        .response_with::<409, Json<ApiErrorResponse>, _>(|r| r.description("User already a member"))
        .security_requirement("bearer_auth")
}

fn update_member_role_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Update member role")
        .description(
            "Change a member's role. \
           Cannot promote to Owner. \
           Requires Owner or Admin role. \
           Admins cannot change other Admins.",
        )
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<WorkspaceMemberWithUser>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| r.description("Forbidden"))
        .security_requirement("bearer_auth")
}

fn remove_member_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Remove a member")
        .description(
            "Remove a member from the workspace. \
           Members can remove themselves. \
           Owner cannot be removed. \
           Requires Owner or Admin role to remove others.",
        )
        .tag("workspaces")
        .response::<200, Json<ApiResponseSchema<bool>>>()
        .response_with::<403, Json<ApiErrorResponse>, _>(|r| r.description("Forbidden"))
        .security_requirement("bearer_auth")
}
