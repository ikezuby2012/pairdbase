use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboxEvent {
    EmailWelcome {
        to: String,
        display_name: String,
        verify_token: String,
    },

    EmailPasswordReset {
        to: String,
        display_name: String,
        reset_token: String,
    },

    EmailMagicLink {
        to: String,
        display_name: String,
        magic_token: String,
    },

    EmailOrgInvitation {
        to: String,
        inviter_name: String,
        org_name: String,
        role: String,
        token: String,
    },

    EmailWorkspaceInvitation {
        to: String,
        display_name: String,
        inviter_name: String,
        org_name: String,
        workspace_name: String,
        role: String,
        workspace_id: Uuid,
    },

    EmailPlanLimitWarning {
        to: String,
        org_name: String,
        resource: String,
        used: i64,
        limit: i64,
    },

    EmailConnectionAlert {
        to: String,
        org_name: String,
        connection_name: String,
        error_message: String,
    },
}

impl OutboxEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::EmailWelcome              { .. } => "email.welcome",
            Self::EmailPasswordReset        { .. } => "email.password_reset",
            Self::EmailMagicLink            { .. } => "email.magic_link",
            Self::EmailOrgInvitation        { .. } => "email.org_invitation",
            Self::EmailWorkspaceInvitation  { .. } => "email.workspace_invitation",
            Self::EmailPlanLimitWarning     { .. } => "email.plan_limit_warning",
            Self::EmailConnectionAlert      { .. } => "email.connection_alert",
        }
    }
}