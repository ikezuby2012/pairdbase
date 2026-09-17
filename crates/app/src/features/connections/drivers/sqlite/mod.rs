use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::time::Instant;

use crate::features::connections::{
    domain::Connection,
    drivers::{
        trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
        versions::{ServerVersion, VersionCapabilities},
    },
};

mod queries;
mod schema;

pub struct SQLiteDriver {
    pool:    SqlitePool,
    version: ServerVersion,
    caps:    VersionCapabilities,
}

impl SQLiteDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let path = conn.database_name.as_deref().unwrap_or(":memory:");

        // Validate file exists before trying to open
        if path != ":memory:" && !std::path::Path::new(path).exists() {
            return Err(format!("SQLite file not found: {}", path));
        }

        let url = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite:{}", path)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(1)    // SQLite is single-writer
            .connect(&url)
            .await
            .map_err(|e| e.to_string())?;

        let version = Self::detect_version(&pool).await?;
        let caps    = VersionCapabilities::for_sqlite(&version);

        tracing::info!(
            version = %version.raw,
            path    = path,
            "SQLite connected"
        );

        Ok(Self { pool, version, caps })
    }

    async fn detect_version(pool: &SqlitePool) -> Result<ServerVersion, String> {
        let (raw,): (String,) = sqlx::query_as("SELECT sqlite_version()")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(ServerVersion::parse(&raw))
    }
}

#[async_trait]
impl DatabaseDriver for SQLiteDriver {
    fn version(&self)      -> &ServerVersion      { &self.version }
    // fn capabilities(&self) -> &VersionCapabilities { &self.caps }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();

        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let version_str = format!("SQLite {}", self.version.raw);
        Ok((version_str, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let (tables, views, triggers) = tokio::try_join!(
            schema::fetch_tables(&self.pool, &self.caps),
            schema::fetch_views(&self.pool, &self.caps),
            schema::fetch_triggers(&self.pool, &self.caps),
        )?;

        let schemas = schema::build_catalogue(tables, views, triggers);

        Ok(DatabaseMetadata::Relational(SchemaInfo {
            database_name:  format!("SQLite {}", self.version.raw),
            schemas,
            scheduled_jobs: vec![],   // SQLite has no scheduler
        }))
    }
}