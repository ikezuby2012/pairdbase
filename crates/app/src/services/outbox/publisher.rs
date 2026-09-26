use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::db::DbPool;

use super::events::OutboxEvent;

#[derive(Clone)]
pub struct OutboxPublisher {
    pool: DbPool,
}

impl OutboxPublisher {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn publish_in_tx(
        event: &OutboxEvent,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        let payload = serde_json::to_value(event).expect("OutboxEvent must be serialisable");

        sqlx::query!(
            r#"
            INSERT INTO TBL_OUTBOX_EVENTS (id, event_type, payload)
            VALUES ($1, $2, $3)
            "#,
            id,
            event.event_type(),
            payload,
        )
        .execute(&mut **tx)
        .await?;

        Ok(id)
    }

    pub async fn publish(&self, event: &OutboxEvent) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        let payload = serde_json::to_value(event).expect("OutboxEvent must be serialisable");

        sqlx::query!(
            r#"
            INSERT INTO TBL_OUTBOX_EVENTS (id, event_type, payload)
            VALUES ($1, $2, $3)
            "#,
            id,
            event.event_type(),
            payload,
        )
        .execute(&self.pool)
        .await?;

        Ok(id)
    }
}
