use async_trait::async_trait;

use super::types::{DeliveryResult, EmailError, EmailMessage};

// ── Provider trait ────────────────────────────────────────────────────────────

#[async_trait]
pub trait EmailProvider: Send + Sync {
    async fn send(&self, msg: &EmailMessage) -> Result<DeliveryResult, EmailError>;
    fn name(&self) -> &'static str;
}

// ── SMTP via lettre ───────────────────────────────────────────────────────────

use lettre::{
    message::{
        header::{ContentType, From, ReplyTo, To},
        Mailbox, Mailboxes, Message, MultiPart, SinglePart,
    },
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{Tls, TlsParameters},
        PoolConfig,
    },
    Address, AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};

pub struct SmtpProvider {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from_box: Mailbox,
}

/// Which TLS mode to use on the SMTP connection
#[derive(Debug, Clone)]
pub enum SmtpTls {
    /// Plain connection — no TLS (only for local dev / mailhog)
    None,
    /// STARTTLS upgrade on port 587 (recommended for production SMTP relays)
    StartTls,
    /// TLS from the start on port 465
    Tls,
}

impl SmtpProvider {
    pub fn new(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        from_email: &str,
        from_name: &str,
        tls: SmtpTls,
    ) -> Result<Self, EmailError> {
        // Parse the from address once at startup — fail fast on bad config
        let from_addr: Address =
            from_email
                .parse()
                .map_err(|e: lettre::address::AddressError| {
                    EmailError::InvalidAddress(format!("from address: {}", e))
                })?;

        let from_box = Mailbox::new(Some(from_name.to_string()), from_addr);

        let creds = Credentials::new(username.to_string(), password.to_string());

        let mailer: AsyncSmtpTransport<Tokio1Executor> = match tls {
            SmtpTls::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
                .map_err(|e| EmailError::Provider(format!("SMTP relay: {}", e)))?
                .port(port)
                .credentials(creds)
                .authentication(vec![Mechanism::Plain, Mechanism::Login])
                .pool_config(PoolConfig::new().min_idle(1).max_size(5))
                .build(),

            SmtpTls::Tls => {
                let tls_params = TlsParameters::new(host.to_string())
                    .map_err(|e| EmailError::Provider(format!("TLS params: {}", e)))?;

                AsyncSmtpTransport::<Tokio1Executor>::relay(host)
                    .map_err(|e| EmailError::Provider(format!("SMTP relay: {}", e)))?
                    .port(port)
                    .tls(Tls::Wrapper(tls_params))
                    .credentials(creds)
                    .authentication(vec![Mechanism::Plain, Mechanism::Login])
                    .pool_config(PoolConfig::new().min_idle(1).max_size(5))
                    .build()
            }

            SmtpTls::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                .port(port)
                .credentials(creds)
                .build(),
        };

        Ok(Self { mailer, from_box })
    }

    /// Build from env vars — called from EmailService::new()
    pub fn from_config(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        from_email: &str,
        from_name: &str,
        tls_mode: &str,
    ) -> Result<Self, EmailError> {
        let tls = match tls_mode.to_lowercase().as_str() {
            "starttls" | "start_tls" => SmtpTls::StartTls,
            "tls" | "ssl" => SmtpTls::Tls,
            "none" | "plain" => SmtpTls::None,
            other => {
                tracing::warn!(
                    tls_mode = other,
                    "unknown SMTP_TLS value — defaulting to STARTTLS"
                );
                SmtpTls::StartTls
            }
        };

        Self::new(host, port, username, password, from_email, from_name, tls)
    }

    // ── Build a lettre Message from our EmailMessage ──────────────────────────

    fn build_message(&self, msg: &EmailMessage) -> Result<Message, EmailError> {
        // ── Recipients ────────────────────────────────────────────────────────
        let mut to_boxes = Mailboxes::new();

        for addr in &msg.to {
            let parsed: Address =
                addr.email
                    .parse()
                    .map_err(|e: lettre::address::AddressError| {
                        EmailError::InvalidAddress(format!("{}: {}", addr.email, e))
                    })?;
            to_boxes.push(Mailbox::new(addr.name.clone(), parsed));
        }

        // ── Builder chain ─────────────────────────────────────────────────────
        let mut builder = Message::builder()
            .from(self.from_box.clone())
            .subject(&msg.subject);

        for mailbox in to_boxes {
            builder = builder.to(mailbox);
        }

        // Optional reply-to
        if let Some(reply) = &msg.reply_to {
            let addr: Address =
                reply
                    .email
                    .parse()
                    .map_err(|e: lettre::address::AddressError| {
                        EmailError::InvalidAddress(format!("{}: {}", reply.email, e))
                    })?;
            let reply_box = Mailbox::new(reply.name.clone(), addr);
            builder = builder.reply_to(reply_box);
        }

        // ── Multipart body: text/plain + text/html ────────────────────────────
        let email = builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(msg.text.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(msg.html.clone()),
                    ),
            )
            .map_err(|e| EmailError::Provider(format!("build message: {}", e)))?;

        Ok(email)
    }
}

#[async_trait]
impl EmailProvider for SmtpProvider {
    fn name(&self) -> &'static str {
        "smtp"
    }

    async fn send(&self, msg: &EmailMessage) -> Result<DeliveryResult, EmailError> {
        let email = self.build_message(msg)?;

        let response = self.mailer.send(email).await.map_err(|e| {
            tracing::error!(error = %e, "SMTP send failed");
            EmailError::Provider(format!("SMTP send: {}", e))
        })?;

        // lettre SmtpResponse carries the server message codes
        let message_id = response.message().next().map(|s| s.to_string());

        tracing::debug!(
            code       = response.code().to_string().as_str(),
            message_id = ?message_id,
            "SMTP accepted"
        );

        Ok(DeliveryResult {
            message_id,
            provider: "smtp".into(),
        })
    }
}

// ── Resend ────────────────────────────────────────────────────────────────────

pub struct ResendProvider {
    api_key: String,
    from_email: String,
    from_name: String,
    http: reqwest::Client,
}

impl ResendProvider {
    pub fn new(api_key: String, from_email: String, from_name: String) -> Self {
        Self {
            api_key,
            from_email,
            from_name,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmailProvider for ResendProvider {
    fn name(&self) -> &'static str {
        "resend"
    }

    async fn send(&self, msg: &EmailMessage) -> Result<DeliveryResult, EmailError> {
        let to: Vec<String> = msg.to.iter().map(|a| a.email.clone()).collect();

        let body = serde_json::json!({
            "from":    format!("{} <{}>", self.from_name, self.from_email),
            "to":      to,
            "subject": msg.subject,
            "html":    msg.html,
            "text":    msg.text,
            "tags":    msg.tags.iter()
                          .map(|t| serde_json::json!({ "name": t }))
                          .collect::<Vec<_>>(),
        });

        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmailError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(EmailError::Provider(format!("Resend {} — {}", status, err)));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| EmailError::Provider(e.to_string()))?;

        Ok(DeliveryResult {
            message_id: data["id"].as_str().map(|s| s.to_string()),
            provider: "resend".into(),
        })
    }
}

// ── SendGrid ──────────────────────────────────────────────────────────────────

pub struct SendGridProvider {
    api_key: String,
    from_email: String,
    from_name: String,
    http: reqwest::Client,
}

impl SendGridProvider {
    pub fn new(api_key: String, from_email: String, from_name: String) -> Self {
        Self {
            api_key,
            from_email,
            from_name,
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmailProvider for SendGridProvider {
    fn name(&self) -> &'static str {
        "sendgrid"
    }

    async fn send(&self, msg: &EmailMessage) -> Result<DeliveryResult, EmailError> {
        let to: Vec<serde_json::Value> = msg
            .to
            .iter()
            .map(|a| {
                serde_json::json!({
                    "email": a.email,
                    "name":  a.name.clone().unwrap_or_default(),
                })
            })
            .collect();

        let body = serde_json::json!({
            "personalizations": [{ "to": to }],
            "from": {
                "email": self.from_email,
                "name":  self.from_name,
            },
            "subject": msg.subject,
            "content": [
                { "type": "text/plain", "value": msg.text },
                { "type": "text/html",  "value": msg.html },
            ],
        });

        let resp = self
            .http
            .post("https://api.sendgrid.com/v3/mail/send")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| EmailError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(EmailError::Provider(format!(
                "SendGrid {} — {}",
                status, err
            )));
        }

        let message_id = resp
            .headers()
            .get("x-message-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        Ok(DeliveryResult {
            message_id,
            provider: "sendgrid".into(),
        })
    }
}

// ── Console (dev / test) ──────────────────────────────────────────────────────

pub struct ConsoleProvider;

#[async_trait]
impl EmailProvider for ConsoleProvider {
    fn name(&self) -> &'static str {
        "console"
    }

    async fn send(&self, msg: &EmailMessage) -> Result<DeliveryResult, EmailError> {
        let to = msg
            .to
            .iter()
            .map(|a| a.display())
            .collect::<Vec<_>>()
            .join(", ");

        tracing::info!(
            to      = %to,
            subject = %msg.subject,
            "\n╔══════════════════════════════════════╗\n\
               ║  EMAIL (console provider)            ║\n\
               ╠══════════════════════════════════════╣\n\
               ║  To:      {to}\n\
               ║  Subject: {subject}\n\
               ╠══════════════════════════════════════╣\n\
               {text}\n\
               ╚══════════════════════════════════════╝",
            to      = to,
            subject = msg.subject,
            text    = msg.text,
        );

        Ok(DeliveryResult {
            message_id: Some(format!("console-{}", uuid::Uuid::new_v4())),
            provider: "console".into(),
        })
    }
}
