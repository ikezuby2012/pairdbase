use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ── Request DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWorkspaceRequest {
    /// Workspace display name
    pub name:        String,
    pub description: Option<String>,
    /// Hex color for UI
    pub color:       Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateWorkspaceRequest {
    pub name:        Option<String>,
    pub description: Option<String>,
    pub color:       Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteMemberRequest {
    /// Email address of the user to invite
    pub email: String,
    /// owner | admin | member | viewer
    pub role:  String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMemberRoleRequest {
    /// owner | admin | member | viewer
    pub role: String,
}