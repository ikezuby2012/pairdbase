use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use shared::{ConnectionId, WorkspaceId};
use uuid::Uuid;

use super::connection_error::ConnectionError;
use super::domain::Connection;
use crate::db::DbPool;
use crate::features::connections::domain::{ConnectionRow, DbType};
use crate::features::connections::vault::{self, Vault};

#[async_trait]
pub trait ConnectionRepo: Send + Sync {
    async fn find_by_id(&self, id: ConnectionId) -> Result<Option<Connection>, ConnectionError>;
    async fn list_for_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Connection>, ConnectionError>;
    async fn create(&self, connection: Connection) -> Result<Connection, ConnectionError>;
    async fn update(&self, connection: Connection) -> Result<Connection, ConnectionError>;
    async fn delete(&self, id: ConnectionId) -> Result<(), ConnectionError>;
}

pub struct PgConnectionRepo {
    pool: DbPool,
    pub vault: Arc<Vault>,
}

impl PgConnectionRepo {
    pub fn new(pool: DbPool, vault: Arc<Vault>) -> Self {
        Self { pool, vault }
    }

    fn decrypt_optional(&self, value: Option<Vec<u8>>) -> Result<Option<String>, ConnectionError> {
        value
            .map(|bytes| {
                self.vault
                    .decrypt(&bytes)
                    .map_err(|e| ConnectionError::Vault(e.to_string()))
            })
            .transpose()
    }

    fn map_connection(&self, row: ConnectionRow) -> Result<Connection, ConnectionError> {
        Ok(Connection {
            id: row.id,
            organization_id: row.organization_id,
            workspace_id: row.workspace_id,

            name: row.name,

            db_type: DbType::from_str(&row.db_type)
                .map_err(|e| ConnectionError::InvalidDbType(e.to_string()))?,

            host: row.host,
            port: row.port.map(|v| v as u16),
            database_name: row.database_name,
            username: row.username,

            password: self.decrypt_optional(row.password)?,

            ssl_mode: row.ssl_mode,

            ssl_ca_cert: self.decrypt_optional(row.ssl_ca_cert)?,
            ssl_client_cert: self.decrypt_optional(row.ssl_client_cert)?,
            ssl_client_key: self.decrypt_optional(row.ssl_client_key)?,

            ssh_enabled: row.ssh_enabled,
            ssh_host: row.ssh_host,
            ssh_port: row.ssh_port.map(|v| v as u16),
            ssh_user: row.ssh_user,

            ssh_private_key: self.decrypt_optional(row.ssh_private_key)?,

            read_only: row.read_only,
            color: row.color,

            pool_min: row.pool_min as u32,
            pool_max: row.pool_max as u32,

            created_by: row.created_by,
            created_at: row.created_at,

            updated_by: row.updated_by,
            updated_at: row.updated_at,

            is_soft_deleted: row.is_soft_deleted,
            deleted_by: row.deleted_by,
            deleted_at: row.deleted_at,
        })
    }
}

#[async_trait]
impl ConnectionRepo for PgConnectionRepo {
    async fn find_by_id(&self, id: ConnectionId) -> Result<Option<Connection>, ConnectionError> {
        let row = sqlx::query_as!(
            ConnectionRow,
            r#"
        SELECT
            id,
            organization_id,
            workspace_id,
            name,
            db_type,

            host,
            port,
            database_name,
            username,

            password,

            ssl_mode,
            ssl_ca_cert,
            ssl_client_cert,
            ssl_client_key,

            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_user,
            ssh_private_key,

            read_only,
            color,

            pool_min,
            pool_max,

            created_by,
            created_at,

            updated_by,
            updated_at,

            is_soft_deleted,
            deleted_by,
            deleted_at

        FROM tbl_db_connections
        WHERE id = $1
          AND is_soft_deleted = FALSE
          AND deleted_at IS NULL
        "#,
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ConnectionError::Database(e.to_string()))?;

        row.map(|row| self.map_connection(row)).transpose()
    }

    async fn list_for_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<Connection>, ConnectionError> {
        let rows = sqlx::query_as!(
            ConnectionRow,
            r#"
        SELECT
            id,
            organization_id,
            workspace_id,
            name,
            db_type,

            host,
            port,
            database_name,
            username,

            password,

            ssl_mode,
            ssl_ca_cert,
            ssl_client_cert,
            ssl_client_key,

            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_user,
            ssh_private_key,

            read_only,
            color,

            pool_min,
            pool_max,

            created_by,
            created_at,

            updated_by,
            updated_at,

            is_soft_deleted,
            deleted_by,
            deleted_at

        FROM tbl_db_connections
        WHERE workspace_id = $1
          AND is_soft_deleted = FALSE
          AND deleted_at IS NULL
        ORDER BY created_at DESC
        "#,
            workspace_id.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConnectionError::Database(e.to_string()))?;

        rows.into_iter()
            .map(|row| self.map_connection(row))
            .collect()
    }

    async fn create(&self, conn: Connection) -> Result<Connection, ConnectionError> {
        let password = self
            .vault
            .encrypt_opt(conn.password.as_deref())
            .map_err(ConnectionError::Vault)?;

        let ssl_ca_cert = self
            .vault
            .encrypt_opt(conn.ssl_ca_cert.as_deref())
            .map_err(ConnectionError::Vault)?;

        let ssl_client_cert = self
            .vault
            .encrypt_opt(conn.ssl_client_cert.as_deref())
            .map_err(ConnectionError::Vault)?;

        let ssl_client_key = self
            .vault
            .encrypt_opt(conn.ssl_client_key.as_deref())
            .map_err(ConnectionError::Vault)?;

        let ssh_private_key = self
            .vault
            .encrypt_opt(conn.ssh_private_key.as_deref())
            .map_err(ConnectionError::Vault)?;

        sqlx::query!(
            r#"
        INSERT INTO tbl_db_connections (
            id,
            organization_id,
            workspace_id,
            name,
            db_type,
            host,
            port,
            database_name,
            username,
            password,
            ssl_mode,
            ssl_ca_cert,
            ssl_client_cert,
            ssl_client_key,
            ssh_enabled,
            ssh_host,
            ssh_port,
            ssh_user,
            ssh_private_key,
            read_only,
            color,
            pool_min,
            pool_max,
            created_by
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            $12,
            $13,
            $14,
            $15,
            $16,
            $17,
            $18,
            $19,
            $20,
            $21,
            $22,
            $23,
            $24
        )
        "#,
            conn.id,
            conn.organization_id,
            conn.workspace_id,
            conn.name,
            conn.db_type.as_str(),
            conn.host,
            conn.port.map(|p| p as i32),
            conn.database_name,
            conn.username,
            password,
            conn.ssl_mode,
            ssl_ca_cert,
            ssl_client_cert,
            ssl_client_key,
            conn.ssh_enabled,
            conn.ssh_host,
            conn.ssh_port.map(|p| p as i32),
            conn.ssh_user,
            ssh_private_key,
            conn.read_only,
            conn.color,
            conn.pool_min as i32,
            conn.pool_max as i32,
            conn.created_by
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectionError::Database(e.to_string()))?;

        Ok(conn)
    }

    async fn update(&self, conn: Connection) -> Result<Connection, ConnectionError> {
        sqlx::query!(
            r#"
            UPDATE tbl_db_connections
            SET
                name = $1,
                read_only = $2,
                color = $3,
                updated_by = $4,
                updated_at = NOW()
            WHERE id = $5
              AND deleted_at IS NULL
            "#,
            conn.name,
            conn.read_only,
            conn.color,
            conn.updated_by,
            conn.id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectionError::Database(e.to_string()))?;

        Ok(conn)
    }

    async fn delete(&self, id: ConnectionId) -> Result<(), ConnectionError> {
        sqlx::query!(
            r#"
            UPDATE tbl_db_connections
            SET
                deleted_at = NOW()
            WHERE id = $1
              AND deleted_at IS NULL
            "#,
            id.0
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ConnectionError::Database(e.to_string()))?;

        Ok(())
    }
}
