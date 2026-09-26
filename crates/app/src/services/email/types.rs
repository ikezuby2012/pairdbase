use serde::{Deserialize, Serialize};

// ── Address ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmailAddress {
    pub email: String,
    pub name: Option<String>,
}

impl EmailAddress {
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: None,
        }
    }

    pub fn named(email: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: Some(name.into()),
        }
    }

    pub fn display(&self) -> String {
        match &self.name {
            Some(n) => format!("{} <{}>", n, self.email),
            None => self.email.clone(),
        }
    }
}

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to: Vec<EmailAddress>,
    pub subject: String,
    pub html: String,
    pub text: String, // plain text fallback
    pub reply_to: Option<EmailAddress>,
    pub tags: Vec<String>,
}

impl EmailMessage {
    pub fn to(
        recipient: EmailAddress,
        subject: impl Into<String>,
        html: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            to: vec![recipient],
            subject: subject.into(),
            html: html.into(),
            text: text.into(),
            reply_to: None,
            tags: vec![],
        }
    }

    pub fn with_reply_to(mut self, addr: EmailAddress) -> Self {
        self.reply_to = Some(addr);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

// ── Delivery result ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DeliveryResult {
    pub message_id: Option<String>,
    pub provider: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("template render error: {0}")]
    Template(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),
}

shared::impl_api_error!(EmailError {
    EmailError::Template(_) => (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "EMAIL_TEMPLATE_ERROR",
        "Failed to render email template".to_string()
    ),

    EmailError::Provider(_) => (
        axum::http::StatusCode::BAD_GATEWAY,
        "EMAIL_PROVIDER_ERROR",
        "Email provider request failed".to_string()
    ),

    EmailError::InvalidAddress(_) => (
        axum::http::StatusCode::BAD_REQUEST,
        "INVALID_EMAIL_ADDRESS",
        "Invalid email address".to_string()
    ),
});
