use std::sync::Arc;
use uuid::Uuid;

pub mod provider;
pub mod renderer;
pub mod templates;
pub mod types;

pub use types::*;

use provider::{
    ConsoleProvider, EmailProvider, ResendProvider, SendGridProvider, SmtpProvider, SmtpTls,
};
use renderer::render;

use crate::config::{EmailConfig, EmailProviderKind};
use crate::vars;

pub struct EmailService {
    provider: Arc<dyn EmailProvider>,
    config: EmailConfig,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Result<Self, EmailError> {
        let provider: Arc<dyn EmailProvider> = match &config.provider {
            EmailProviderKind::Smtp => {
                let host = config
                    .smtp_host
                    .as_deref()
                    .ok_or_else(|| EmailError::Provider("SMTP_HOST not set".into()))?;
                let port = config.smtp_port.unwrap_or(587);
                let user = config.smtp_username.as_deref().unwrap_or("");
                let pass = config.smtp_password.as_deref().unwrap_or("");

                Arc::new(SmtpProvider::new(
                    host,
                    port,
                    user,
                    pass,
                    &config.from_email,
                    &config.from_name,
                    SmtpTls::Tls,
                )?)
            }
            EmailProviderKind::Resend => {
                let key = config
                    .resend_api_key
                    .clone()
                    .ok_or_else(|| EmailError::Provider("RESEND_API_KEY not set".into()))?;
                Arc::new(ResendProvider::new(
                    key,
                    config.from_email.clone(),
                    config.from_name.clone(),
                ))
            }
            EmailProviderKind::SendGrid => {
                let key = config
                    .sendgrid_api_key
                    .clone()
                    .ok_or_else(|| EmailError::Provider("SENDGRID_API_KEY not set".into()))?;
                Arc::new(SendGridProvider::new(
                    key,
                    config.from_email.clone(),
                    config.from_name.clone(),
                ))
            }
            EmailProviderKind::Console => Arc::new(ConsoleProvider),
        };

        Ok(Self { provider, config })
    }

    pub async fn send(&self, msg: EmailMessage) -> Result<types::DeliveryResult, EmailError> {
        tracing::debug!(
            provider = self.provider.name(),
            to       = msg.to.iter().map(|a| a.email.as_str()).collect::<Vec<_>>().join(", "),
            subject  = %msg.subject,
            "sending email"
        );
        let result = self.provider.send(&msg).await?;
        tracing::info!(
            provider   = %result.provider,
            message_id = ?result.message_id,
            subject    = %msg.subject,
            "email delivered"
        );
        Ok(result)
    }

    // ── Template helpers ──────────────────────────────────────────────────────

    /// Send welcome + email verification email after registration
    pub async fn send_welcome(
        &self,
        to: &str,
        display_name: &str,
        verify_token: &str,
    ) -> Result<(), EmailError> {
        let verify_url = format!(
            "{}/auth/verify-email?token={}",
            self.config.base_url, verify_token
        );

        let vars = vars![
            "display_name" => display_name,
            "verify_url"   => &verify_url,
        ];

        let html = render(templates::WELCOME, &vars);
        let text = format!(
            "Welcome to Pairdbase, {}!\n\nVerify your email: {}\n\n\
             This link expires in 24 hours.",
            display_name, verify_url
        );

        self.send(
            EmailMessage::to(
                EmailAddress::named(to, display_name),
                "Welcome to Pairdbase — verify your email",
                html,
                text,
            )
            .with_tag("welcome"),
        )
        .await?;

        Ok(())
    }

    // ── Password reset ────────────────────────────────────────────────────────

    pub async fn send_password_reset(
        &self,
        to: &str,
        display_name: &str,
        reset_token: &str,
    ) -> Result<(), EmailError> {
        let reset_url = format!(
            "{}/auth/reset-password?token={}",
            self.config.base_url, reset_token
        );

        let vars = vars![
            "display_name" => display_name,
            "reset_url"    => &reset_url,
        ];

        let html = render(templates::PASSWORD_RESET, &vars);
        let text = format!(
            "Hi {}!\n\nReset your password: {}\n\n\
             This link expires in 1 hour.",
            display_name, reset_url
        );

        self.send(
            EmailMessage::to(
                EmailAddress::named(to, display_name),
                "Reset your Pairdbase password",
                html,
                text,
            )
            .with_tag("password-reset"),
        )
        .await?;

        Ok(())
    }

    // ── Org invitation ────────────────────────────────────────────────────────

    pub async fn send_org_invitation(
        &self,
        to: &str,
        inviter_name: &str,
        org_name: &str,
        role: &str,
        token: &str,
    ) -> Result<(), EmailError> {
        let accept_url = format!(
            "{}/invitations/accept?token={}",
            self.config.base_url, token
        );

        let vars = vars![
            "inviter_name" => inviter_name,
            "org_name"     => org_name,
            "role"         => role,
            "accept_url"   => &accept_url,
            "expires_days" => "7",
        ];

        let html = render(templates::ORG_INVITATION, &vars);
        let subject = format!(
            "{} invited you to join {} on Pairdbase",
            inviter_name, org_name
        );
        let text = format!(
            "{} invited you to join {} as a {}.\n\nAccept: {}\n\nExpires in 7 days.",
            inviter_name, org_name, role, accept_url
        );

        self.send(
            EmailMessage::to(EmailAddress::new(to), subject, html, text).with_tag("org-invitation"),
        )
        .await?;

        Ok(())
    }

    // ── Workspace invitation ──────────────────────────────────────────────────

    pub async fn send_workspace_invitation(
        &self,
        to: &str,
        display_name: &str,
        inviter_name: &str,
        org_name: &str,
        workspace_name: &str,
        role: &str,
        workspace_id: Uuid,
    ) -> Result<(), EmailError> {
        let workspace_url = format!("{}/workspaces/{}", self.config.base_url, workspace_id);

        let vars = vars![
            "inviter_name"   => inviter_name,
            "org_name"       => org_name,
            "workspace_name" => workspace_name,
            "role"           => role,
            "workspace_url"  => &workspace_url,
        ];

        let html = render(templates::WORKSPACE_INVITATION, &vars);
        let subject = format!(
            "{} added you to the {} workspace",
            inviter_name, workspace_name
        );
        let text = format!(
            "{} added you to {} in {} as a {}.\n\nOpen workspace: {}",
            inviter_name, workspace_name, org_name, role, workspace_url
        );

        self.send(
            EmailMessage::to(EmailAddress::named(to, display_name), subject, html, text)
                .with_tag("workspace-invitation"),
        )
        .await?;

        Ok(())
    }

    // ── Magic link ────────────────────────────────────────────────────────────

    pub async fn send_magic_link(
        &self,
        to: &str,
        display_name: &str,
        magic_token: &str,
    ) -> Result<(), EmailError> {
        let magic_url = format!("{}/auth/magic?token={}", self.config.base_url, magic_token);

        let vars = vars![
            "display_name" => display_name,
            "magic_url"    => &magic_url,
        ];

        let html = render(templates::MAGIC_LINK, &vars);
        let text = format!(
            "Hi {}!\n\nSign in to Pairdbase: {}\n\n\
             This link expires in 15 minutes and can only be used once.",
            display_name, magic_url
        );

        self.send(
            EmailMessage::to(
                EmailAddress::named(to, display_name),
                "Your Pairdbase sign-in link",
                html,
                text,
            )
            .with_tag("magic-link"),
        )
        .await?;

        Ok(())
    }

    // ── Plan limit warning ────────────────────────────────────────────────────

    pub async fn send_plan_limit_warning(
        &self,
        to: &str,
        org_name: &str,
        resource: &str,
        used: i64,
        limit: i64,
    ) -> Result<(), EmailError> {
        let upgrade_url = format!("{}/settings/billing", self.config.base_url);
        let percent = format!("{}", used * 100 / limit.max(1));
        let used_s = used.to_string();
        let limit_s = limit.to_string();

        let vars = vars![
            "org_name"    => org_name,
            "resource"    => resource,
            "used"        => &used_s,
            "limit"       => &limit_s,
            "percent"     => &percent,
            "upgrade_url" => &upgrade_url,
        ];

        let html = render(templates::PLAN_LIMIT, &vars);
        let subject = format!("{} is at {}% of its {} limit", org_name, percent, resource);
        let text = format!(
            "{} is at {}% of its {} limit ({}/{}).\n\nUpgrade: {}",
            org_name, percent, resource, used, limit, upgrade_url
        );

        self.send(
            EmailMessage::to(EmailAddress::new(to), subject, html, text).with_tag("plan-limit"),
        )
        .await?;

        Ok(())
    }

    // ── Connection alert ──────────────────────────────────────────────────────

    pub async fn send_connection_alert(
        &self,
        to: &str,
        org_name: &str,
        connection_name: &str,
        error_message: &str,
    ) -> Result<(), EmailError> {
        let dashboard_url = format!("{}/connections", self.config.base_url);

        let vars = vars![
            "org_name"        => org_name,
            "connection_name" => connection_name,
            "error_message"   => error_message,
            "dashboard_url"   => &dashboard_url,
        ];

        let html = render(templates::CONNECTION_ALERT, &vars);
        let subject = format!("Connection alert: {} is unreachable", connection_name);
        let text = format!(
            "Connection alert: {} in {} is unreachable.\n\nError: {}\n\nDashboard: {}",
            connection_name, org_name, error_message, dashboard_url
        );

        self.send(
            EmailMessage::to(EmailAddress::new(to), subject, html, text)
                .with_tag("connection-alert"),
        )
        .await?;

        Ok(())
    }
}
