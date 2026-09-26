use std::sync::Arc;
use tokio::time::{interval, Duration};

use super::events::OutboxEvent;
use crate::services::email::EmailService;

use crate::db::DbPool;

const POLL_INTERVAL_SECS: u64 = 5;
const BATCH_SIZE: i64 = 50;
const MAX_ATTEMPTS: i32 = 5;

pub struct OutboxWorker {
    pool: DbPool,
    email: Arc<EmailService>,
}

impl OutboxWorker {
    pub fn new(pool: DbPool, email: Arc<EmailService>) -> Self {
        Self { pool, email }
    }

    /// Spawn the worker as a background task.
    /// Call once at startup from main.rs.
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&self) {
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));

        loop {
            ticker.tick().await;

            if let Err(e) = self.process_batch().await {
                tracing::error!(error = %e, "outbox worker error");
            }
        }
    }

    async fn process_batch(&self) -> Result<(), sqlx::Error> {
        // Claim a batch of pending events atomically.
        // SKIP LOCKED means other workers skip rows locked by this instance.
        let rows = sqlx::query!(
            r#"
            UPDATE TBL_OUTBOX_EVENTS
            SET    status   = 'processing',
                   attempts = attempts + 1
            WHERE id IN (
                SELECT id FROM TBL_OUTBOX_EVENTS
                WHERE  status      = 'pending'
                  AND  attempts    < $1
                  AND  scheduled_at <= NOW()
                ORDER BY scheduled_at
                LIMIT  $2
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, event_type, payload, attempts
            "#,
            MAX_ATTEMPTS,
            BATCH_SIZE,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(());
        }

        tracing::debug!(count = rows.len(), "outbox: processing batch");

        for row in rows {
            let event: OutboxEvent = match serde_json::from_value(row.payload) {
                Ok(e) => e,
                Err(e) => {
                    tracing::error!(
                        id    = %row.id,
                        error = %e,
                        "outbox: failed to deserialise event"
                    );
                    self.mark_failed(row.id, &e.to_string()).await?;
                    continue;
                }
            };

            match self.dispatch(&event).await {
                Ok(()) => {
                    self.mark_done(row.id).await?;
                    tracing::info!(
                        id         = %row.id,
                        event_type = %row.event_type,
                        "outbox: event processed"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        id         = %row.id,
                        event_type = %row.event_type,
                        attempts   = row.attempts,
                        error      = %e,
                        "outbox: event failed"
                    );

                    if row.attempts >= MAX_ATTEMPTS {
                        self.mark_failed(row.id, &e).await?;
                    } else {
                        // Exponential backoff — retry in 2^attempts minutes
                        let delay_mins = 1i64 << row.attempts;
                        self.reschedule(row.id, delay_mins, &e).await?;
                    }
                }
            }
        }

        Ok(())
    }

    // ── Dispatch to the correct handler ───────────────────────────────────────

    async fn dispatch(&self, event: &OutboxEvent) -> Result<(), String> {
        match event {
            OutboxEvent::EmailWelcome {
                to,
                display_name,
                verify_token,
            } => self
                .email
                .send_welcome(to, display_name, verify_token)
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailPasswordReset {
                to,
                display_name,
                reset_token,
            } => self
                .email
                .send_password_reset(to, display_name, reset_token)
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailMagicLink {
                to,
                display_name,
                magic_token,
            } => self
                .email
                .send_magic_link(to, display_name, magic_token)
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailOrgInvitation {
                to,
                inviter_name,
                org_name,
                role,
                token,
            } => self
                .email
                .send_org_invitation(to, inviter_name, org_name, role, token)
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailWorkspaceInvitation {
                to,
                display_name,
                inviter_name,
                org_name,
                workspace_name,
                role,
                workspace_id,
            } => self
                .email
                .send_workspace_invitation(
                    to,
                    display_name,
                    inviter_name,
                    org_name,
                    workspace_name,
                    role,
                    *workspace_id,
                )
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailPlanLimitWarning {
                to,
                org_name,
                resource,
                used,
                limit,
            } => self
                .email
                .send_plan_limit_warning(to, org_name, resource, *used, *limit)
                .await
                .map_err(|e| e.to_string()),

            OutboxEvent::EmailConnectionAlert {
                to,
                org_name,
                connection_name,
                error_message,
            } => self
                .email
                .send_connection_alert(to, org_name, connection_name, error_message)
                .await
                .map_err(|e| e.to_string()),
        }
    }

    // ── DB state transitions ──────────────────────────────────────────────────

    async fn mark_done(&self, id: uuid::Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE TBL_OUTBOX_EVENTS
             SET status = 'done', processed_at = NOW()
             WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn mark_failed(&self, id: uuid::Uuid, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE TBL_OUTBOX_EVENTS
             SET status = 'failed', last_error = $1
             WHERE id = $2",
            error,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn reschedule(
        &self,
        id: uuid::Uuid,
        delay_mins: i64,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE TBL_OUTBOX_EVENTS
            SET status       = 'pending',
                last_error   = $1,
                scheduled_at = NOW() + ($2 || ' minutes')::interval
            WHERE id = $3
            "#,
            error,
            delay_mins.to_string(),
            id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
