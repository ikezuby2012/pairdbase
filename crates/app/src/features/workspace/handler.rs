use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use schemars::JsonSchema;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    abstractions::ApiResponse,
    extractors::AuthUser,
    features::{query::error::QueryError, workspace::domain::WorkspaceMemberWithUser},
    state::{AppState, SharedState},
};

use super::{
    domain::WorkspaceView,
    dto::{
        CreateWorkspaceRequest, InviteMemberRequest, UpdateMemberRoleRequest,
        UpdateWorkspaceRequest,
    },
    error::WorkspaceError,
};

pub async fn list(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<ApiResponse<Vec<WorkspaceView>>, WorkspaceError> {
    let workspaces = state.workspaces.list(auth.user_id, auth.org_id).await?;
    Ok(ApiResponse::success(workspaces, "All Workspace"))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<ApiResponse<WorkspaceView>, WorkspaceError> {
    let workspace = state.workspaces.get(workspace_id, auth.user_id).await?;
    Ok(ApiResponse::success(
        workspace,
        "Doc retrieved successfully",
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateWorkspaceRequest>,
) -> Result<ApiResponse<WorkspaceView>, WorkspaceError> {
    let workspace = state
        .workspaces
        .create(body, auth.user_id, auth.org_id)
        .await?;
    Ok(ApiResponse::success(workspace, "Doc created successfully"))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<UpdateWorkspaceRequest>,
) -> Result<ApiResponse<WorkspaceView>, WorkspaceError> {
    let workspace = state
        .workspaces
        .update(workspace_id, body, auth.user_id)
        .await?;
    Ok(ApiResponse::success(workspace, "Doc updated successfully"))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<ApiResponse<bool>, WorkspaceError> {
    state.workspaces.delete(workspace_id, auth.user_id).await?;
    Ok(ApiResponse::success(true, "Doc deleted successfully"))
}

pub async fn list_members(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
) -> Result<ApiResponse<Vec<WorkspaceMemberWithUser>>, WorkspaceError> {
    let members = state
        .workspaces
        .list_members(workspace_id, auth.user_id)
        .await?;
    Ok(ApiResponse::success(
        members,
        "all docs retreived successfully",
    ))
}

pub async fn invite_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(workspace_id): Path<Uuid>,
    Json(body): Json<InviteMemberRequest>,
) -> Result<ApiResponse<WorkspaceMemberWithUser>, WorkspaceError> {
    let member = state
        .workspaces
        .invite_member(workspace_id, body, auth.user_id, auth.org_id)
        .await?;
    Ok(ApiResponse::success(member, "Member invited successfully"))
}

pub async fn update_member_role(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((workspace_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMemberRoleRequest>,
) -> Result<ApiResponse<WorkspaceMemberWithUser>, WorkspaceError> {
    let member = state
        .workspaces
        .update_member_role(workspace_id, target_user_id, body, auth.user_id)
        .await?;
    Ok(ApiResponse::success(member, ""))
}

pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((workspace_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, WorkspaceError> {
    state
        .workspaces
        .remove_member(workspace_id, target_user_id, auth.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
