use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ── Request DTOs ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateOrgRequest {
    /// Organization display name
    pub name:    String,
    /// URL-safe identifier — auto-generated if not provided
    pub slug:    Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateOrgRequest {
    pub name:     Option<String>,
    pub logo_url: Option<String>,
    pub website:  Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteMemberRequest {
    pub email: String,
    /// owner | admin | member
    pub role:  String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMemberRoleRequest {
    pub role: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AcceptInvitationRequest {
    /// The token from the invitation email link
    pub token: String,
}

