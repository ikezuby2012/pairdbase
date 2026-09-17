use shared::{ConnectionId, UserId, WorkspaceId};
use std::sync::Arc;
use uuid::Uuid;

use crate::features::connections::{
    connection_error::ConnectionError,
    domain::{Connection, ConnectionView, DbType, SslMode, TestResult},
    drivers::{
        mongodb::MongoDriver,
        mysql::MySqlDriver,
        oracle::OracleDriver,
        postgres::PostgresDriver,
        sqlserver::SqlServerDriver,
        redis::RedisDriver,
        trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
    },
    pool::{DriverRegistry, SchemaCache},
    repository::ConnectionRepo,
    vault::Vault,
};

pub struct ConnectionUseCases {
    repo: Arc<dyn ConnectionRepo>,
    vault: Arc<Vault>,
    registry: Arc<DriverRegistry>,
    schema_cache: Arc<SchemaCache>,
}

impl ConnectionUseCases {
    pub fn new(
        repo: Arc<dyn ConnectionRepo>,
        vault: Arc<Vault>,
        registry: Arc<DriverRegistry>,
        schema_cache: Arc<SchemaCache>,
    ) -> Self {
        Self {
            repo,
            vault,
            registry,
            schema_cache,
        }
    }

    pub async fn get_full(&self, id: Uuid, org_id: Uuid) -> Result<Connection, ConnectionError> {
        let conn = self
            .repo
            .find_by_id(ConnectionId(id))
            .await?
            .ok_or(ConnectionError::NotFound)?;

        if conn.organization_id != org_id {
            return Err(ConnectionError::Forbidden);
        }

        Ok(conn)
    }

    /// Evict schema cache — called before refresh_schema
    pub async fn invalidate_schema_cache(&self, id: Uuid) {
        self.schema_cache.invalidate(id).await;
        self.registry.remove(id);
    }

    /// Returns db_type without decrypting secrets — used by query handler
    pub async fn get_db_type(&self, id: Uuid) -> Result<DbType, ConnectionError> {
        let conn = self
            .repo
            .find_by_id(ConnectionId(id))
            .await?
            .ok_or(ConnectionError::NotFound)?;

        Ok(conn.db_type)
    }

    pub async fn list(&self, workspace_id: Uuid) -> Result<Vec<ConnectionView>, ConnectionError> {
        let workspace_id = WorkspaceId(workspace_id);
        let conns = self.repo.list_for_workspace(workspace_id).await?;
        Ok(conns.iter().map(ConnectionView::from).collect())
    }

    pub async fn create(&self, conn: Connection) -> Result<ConnectionView, ConnectionError> {
        let workspace_id = WorkspaceId(conn.workspace_id);
        // Business rule: unique name within workspace
        let existing = self.repo.list_for_workspace(workspace_id).await?;
        if existing.iter().any(|c| c.name == conn.name) {
            return Err(ConnectionError::NameTaken);
        }

        let saved = self.repo.create(conn).await?;
        Ok(ConnectionView::from(&saved))
    }

    pub async fn get(&self, id: Uuid) -> Result<ConnectionView, ConnectionError> {
        let conn_id = ConnectionId(id);
        let conn = self
            .repo
            .find_by_id(conn_id)
            .await?
            .ok_or(ConnectionError::NotFound)?;
        Ok(ConnectionView::from(&conn))
    }

    pub async fn delete(&self, id: Uuid, requester: Uuid) -> Result<(), ConnectionError> {
        let conn_id = ConnectionId(id);
        let conn = self
            .repo
            .find_by_id(conn_id)
            .await?
            .ok_or(ConnectionError::NotFound)?;

        if conn.created_by != requester {
            return Err(ConnectionError::Forbidden);
        }

        self.schema_cache.invalidate(id).await;
        self.registry.remove(id);
        self.repo.delete(conn_id).await
    }

    pub async fn update(&self, connection: Connection) -> Result<Connection, ConnectionError> {
        self.repo.update(connection).await
    }

    pub async fn get_schema(&self, id: Uuid) -> Result<DatabaseMetadata, ConnectionError> {
        // 1. Check Redis cache first
        if let Some(cached) = self.schema_cache.get(id).await {
            return Ok(cached);
        }

        // 2. Cache miss — get driver (connect if not already pooled)
        let driver = self.get_or_connect(id).await?;

        // 3. Fetch fresh schema from DB
        let schema = driver
            .fetch_schema()
            .await
            .map_err(|e| ConnectionError::Driver(e))?;

        // 4. Store in Redis for 5 minutes
        self.schema_cache.set(id, &schema).await;

        Ok(schema)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    /// Get driver from registry, or connect and register if not present
    async fn get_or_connect(&self, id: Uuid) -> Result<Arc<dyn DatabaseDriver>, ConnectionError> {
        if let Some(driver) = self.registry.get(id) {
            return Ok(driver);
        }

        // Load from DB and decrypt credentials
        let conn = self
            .repo
            .find_by_id(ConnectionId(id))
            .await?
            .ok_or(ConnectionError::NotFound)?;

        let driver = self
            .build_driver(&conn)
            .await
            .map_err(|e| ConnectionError::Driver(e))?;

        let driver = Arc::from(driver);
        self.registry.register(id, Arc::clone(&driver));

        Ok(driver)
    }

    pub async fn test(&self, conn: &Connection) -> TestResult {
        match self.build_driver(conn).await {
            Ok(driver) => match driver.ping().await {
                Ok((version, latency_ms)) => TestResult {
                    success: true,
                    message: format!("Connected successfully"),
                    latency_ms: Some(latency_ms),
                    server_version: Some(version),
                },
                Err(e) => TestResult {
                    success: false,
                    message: e,
                    latency_ms: None,
                    server_version: None,
                },
            },
            Err(e) => TestResult {
                success: false,
                message: e,
                latency_ms: None,
                server_version: None,
            },
        }
    }

    async fn build_driver(&self, conn: &Connection) -> Result<Box<dyn DatabaseDriver>, String> {
        match conn.db_type {
            DbType::PostgreSQL => Ok(Box::new(PostgresDriver::connect(conn).await?)),
            DbType::MySQL => Ok(Box::new(MySqlDriver::connect(conn).await?)),
            DbType::MongoDB => Ok(Box::new(MongoDriver::connect(conn).await?)),
            DbType::Redis => Ok(Box::new(RedisDriver::connect(conn).await?)), //Ok(Box::new(RedisDriver::connect(conn).await?)),
            DbType::SQLite => Err("SQLite not yet supported".into()),
            DbType::Oracle => Ok(Box::new(OracleDriver::connect(conn).await?)),
            DbType::SqlServer => Ok(Box::new(SqlServerDriver::connect(conn).await?)),
        }
    }
}
