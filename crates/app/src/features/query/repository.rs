use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

use crate::db::DbPool;

use super::domain::{ConnectionPermission, QueryHistoryRecord};
use super::error::QueryError;

#[async_trait]
pub trait QueryRepo: Send + Sync {
    async fn get_permission(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
    ) -> Result<ConnectionPermission, QueryError>;

    async fn save_history(&self, record: QueryHistoryRecord) -> Result<(), QueryError>;
}

pub struct PgQueryRepo {
    pool: DbPool,
}

impl PgQueryRepo {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl QueryRepo for PgQueryRepo {
    async fn get_permission(
        &self,
        connection_id: Uuid,
        user_id: Uuid,
    ) -> Result<ConnectionPermission, QueryError> {
        let row = sqlx::query!(
            r#"
            SELECT
                allow_select,
                allow_insert,
                allow_update,
                allow_delete,
                allow_ddl
            FROM TBL_CONNECTION_PERMISSIONS
            WHERE connection_id = $1
              AND (user_id = $2 OR user_id IS NULL)
            ORDER BY user_id NULLS LAST
            LIMIT 1
            "#,
            connection_id,
            user_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| QueryError::PermissionError(e.to_string()))?;

        Ok(match row {
            Some(r) => ConnectionPermission {
                allow_select: r.allow_select,
                allow_insert: r.allow_insert,
                allow_update: r.allow_update,
                allow_delete: r.allow_delete,
                allow_ddl: r.allow_ddl,
            },
            None => ConnectionPermission::default(),
        })
    }

    async fn save_history(
        &self,
        r: QueryHistoryRecord,
    ) -> Result<(), QueryError> {
        sqlx::query!(
            r#"
            INSERT INTO TBL_QUERY_HISTORY (
                id,
                organization_id,
                workspace_id,
                connection_id,
                user_id,
                query_text,
                query_hash,
                status,
                rejection_reason,
                duration_ms,
                rows_affected,
                error_message,
                executed_at,
                created_by
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13, $14
            )
            "#,
            r.id.0,
            r.organization_id.0,
            r.workspace_id.0,
            r.connection_id.0,
            r.user_id.0,
            r.query_text,
            r.query_hash,
            r.status.as_str(),
            r.rejection_reason,
            r.duration_ms,
            r.rows_affected,
            r.error_message,
            r.executed_at,
            r.user_id.0,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| QueryError::Internal(e.to_string()))?;

        Ok(())
    }
}