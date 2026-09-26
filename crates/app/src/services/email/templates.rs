//! HTML templates embedded at compile time.
//! Each constant is a raw HTML string with {{placeholder}} markers.

pub const WELCOME: &str = include_str!("templates/welcome.html");
pub const PASSWORD_RESET: &str = include_str!("templates/password_reset.html");
pub const ORG_INVITATION: &str = include_str!("templates/org_invitation.html");
pub const WORKSPACE_INVITATION: &str = include_str!("templates/workspace_invitation.html");
pub const MAGIC_LINK: &str = include_str!("templates/magic_link.html");
pub const PLAN_LIMIT: &str = include_str!("templates/plan_limit.html");
pub const CONNECTION_ALERT: &str = include_str!("templates/connection_alert.html");
