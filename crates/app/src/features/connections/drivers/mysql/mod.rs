use async_trait::async_trait;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
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

pub struct MySqlDriver {
    pool: MySqlPool,
    version: ServerVersion,
    caps: VersionCapabilities,
}

impl MySqlDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let host = conn.host.as_deref().unwrap_or("localhost");
        let port = conn.port.unwrap_or(3306);
        let db = conn.database_name.as_deref().unwrap_or("");
        let user = conn.username.as_deref().unwrap_or("root");
        let pass = conn.password.as_deref().unwrap_or("");

        let url = format!("mysql://{user}:{pass}@{host}:{port}/{db}");

        let pool = MySqlPoolOptions::new()
            .min_connections(conn.pool_min)
            .max_connections(conn.pool_max)
            .connect(&url)
            .await
            .map_err(|e| e.to_string())?;

        let version = Self::detect_version(&pool).await?;
        let caps = Self::build_capabilities(&version);

        tracing::info!(
            version  = %version.raw,
            major    = version.major,
            minor    = version.minor,
            is_mariadb       = version.raw.to_lowercase().contains("mariadb"),
            has_window_fns   = caps.window_functions,
            has_sequences    = caps.sequences,
            "MySQL/MariaDB connected"
        );

        Ok(Self {
            pool,
            version,
            caps,
        })
    }

    async fn detect_version(pool: &MySqlPool) -> Result<ServerVersion, String> {
        let (raw,): (String,) = sqlx::query_as("SELECT VERSION()")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(ServerVersion::parse(&raw))
    }

    fn build_capabilities(v: &ServerVersion) -> VersionCapabilities {
        // Distinguish MariaDB from MySQL — they share VERSION() format but diverge
        // MariaDB reports: "10.11.4-MariaDB" or "11.2.3-MariaDB"
        if v.raw.to_lowercase().contains("mariadb") {
            VersionCapabilities::for_mariadb(v)
        } else {
            VersionCapabilities::for_mysql(v)
        }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();
        let (version,): (String,) = sqlx::query_as("SELECT VERSION()")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok((version, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let (tables, views, functions, procedures, sequences, events) = tokio::try_join!(
            schema::fetch_tables(&self.pool, &self.caps),
            schema::fetch_views(&self.pool, &self.caps),
            schema::fetch_functions(&self.pool, &self.caps),
            schema::fetch_procedures(&self.pool, &self.caps),
            schema::fetch_sequences(&self.pool, &self.caps),
            schema::fetch_events(&self.pool, &self.caps),
        )?;

        let schemas = schema::group_by_schema(tables, views, functions, procedures, sequences);

        let db_label = if self.version.raw.to_lowercase().contains("mariadb") {
            format!("MariaDB {}", self.version.raw)
        } else {
            format!("MySQL {}.{}", self.version.major, self.version.minor)
        };

        Ok(DatabaseMetadata::Relational(SchemaInfo {
            database_name: db_label,
            schemas,
            scheduled_jobs: events,
        }))
    }
}
