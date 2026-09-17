use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Instant;

use crate::features::connections::{
    domain::Connection,
    drivers::{
        trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
        versions::{ServerVersion, VersionCapabilities},
    },
};

pub mod queries;
pub mod schema;

pub struct PostgresDriver {
    pool: PgPool,
    version: ServerVersion,
    caps: VersionCapabilities,
}

impl PostgresDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let host = conn.host.as_deref().unwrap_or("localhost");
        let port = conn.port.unwrap_or(5432);
        let db = conn.database_name.as_deref().unwrap_or("postgres");
        let user = conn.username.as_deref().unwrap_or("postgres");
        let pass = conn.password.as_deref().unwrap_or("");

        let url = format!(
            "postgresql://{user}:{pass}@{host}:{port}/{db}?sslmode={}",
            conn.ssl_mode.as_str()
        );

        let pool = PgPoolOptions::new()
            .min_connections(conn.pool_min)
            .max_connections(conn.pool_max)
            .connect(&url)
            .await
            .map_err(|e| e.to_string())?;

        let version = Self::detect_version(&pool).await?;
        let caps = VersionCapabilities::for_postgres(&version);

        tracing::info!(
            version = %version.raw,
            major   = version.major,
            minor   = version.minor,
            has_procedures      = caps.procedures,
            has_generated_cols  = caps.generated_columns,
            "PostgreSQL connected"
        );

        Ok(Self {
            pool,
            version,
            caps,
        })
    }

    async fn detect_version(pool: &PgPool) -> Result<ServerVersion, String> {
        let row: (String,) = sqlx::query_as("SELECT version()")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(ServerVersion::parse(&row.0))
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();
        let (version,): (String,) = sqlx::query_as("SELECT version()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok((version, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let (tables, views, mat_views, functions, procedures, sequences, types, extensions) = tokio::try_join!(
            schema::fetch_tables(&self.pool, &self.caps),
            schema::fetch_views(&self.pool, &self.caps),
            schema::fetch_materialized_views(&self.pool, &self.caps),
            schema::fetch_functions(&self.pool, &self.caps),
            schema::fetch_procedures(&self.pool, &self.caps),
            schema::fetch_sequences(&self.pool, &self.caps),
            schema::fetch_types(&self.pool, &self.caps),
            schema::fetch_extensions(&self.pool, &self.caps),
        )?;

        let schemas = schema::group_by_schema(
            tables, views, mat_views, functions, procedures, sequences, types, extensions,
        );

        Ok(DatabaseMetadata::Relational(SchemaInfo {
            database_name: format!("PostgreSQL {}.{}", self.version.major, self.version.minor),
            schemas,
            scheduled_jobs: vec![], // pg_cron is an extension — not universally available
        }))
    }
}
