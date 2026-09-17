use async_trait::async_trait;
use bb8::Pool;
use std::future::Future;
use std::pin::Pin;
use std::time::Instant;
use tiberius::{AuthMethod, Client, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::features::connections::{
    domain::{Connection, SslMode},
    drivers::{
        trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
        versions::{ServerVersion, SqlServerCapabilities, VersionCapabilities},
    },
};

pub mod queries;
pub mod schema;

struct SqlServerConnectionManager {
    config: Config,
}

impl SqlServerConnectionManager {
    fn from_connection(conn: &Connection) -> Result<Self, String> {
        let mut config = Config::new();

        config.host(conn.host.as_deref().unwrap_or("localhost"));
        config.port(conn.port.unwrap_or(1433));

        if let Some(db) = &conn.database_name {
            config.database(db);
        }

        match (&conn.username, &conn.password) {
            (Some(u), Some(p)) => config.authentication(AuthMethod::sql_server(u, p)),
            _ => config.authentication(AuthMethod::Integrated),
        }

        let ssl_mode = match conn.ssl_mode.as_str() {
            "prefer" => SslMode::Prefer,
            "allow" => SslMode::Allow,
            "require" => SslMode::Require,
            "verify-ca" => SslMode::VerifyCa,
            "verify-full" => SslMode::VerifyFull,
            _ => SslMode::Disable,
        };

        config.encryption(match ssl_mode {
            SslMode::Require | SslMode::VerifyFull | SslMode::VerifyCa => EncryptionLevel::Required,
            SslMode::Disable => EncryptionLevel::NotSupported,
            _ => EncryptionLevel::Off,
        });

        config.trust_cert();

        Ok(Self { config })
    }
}

#[async_trait]
impl bb8::ManageConnection for SqlServerConnectionManager {
    type Connection = Client<Compat<TcpStream>>;
    type Error = tiberius::error::Error;

    fn connect(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Self::Error>> + Send + '_>> {
        let config = self.config.clone();
        Box::pin(async move {
            let tcp = TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;
            Client::connect(config, tcp.compat_write()).await
        })
    }

    fn is_valid<'a>(
        &self,
        conn: &'a mut Self::Connection,
    ) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + Send + 'a>> {
        Box::pin(async move {
            conn.simple_query("SELECT 1").await?.into_results().await?;

            Ok(())
        })
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

pub struct SqlServerDriver {
    pool: Pool<SqlServerConnectionManager>,
    version: ServerVersion,
    caps: VersionCapabilities,
}

impl SqlServerDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let manager = SqlServerConnectionManager::from_connection(conn)?;

        let pool = Pool::builder()
            .min_idle(Some(conn.pool_min))
            .max_size(conn.pool_max)
            .build(manager)
            .await
            .map_err(|e| e.to_string())?;

        let version = Self::detect_version(&pool).await?;
        let caps = VersionCapabilities::for_sqlserver(&version);

        tracing::info!(
            version  = %version.raw,
            product  = SqlServerCapabilities::product_name(&version),
            major    = version.major,
            "SQL Server connected"
        );

        Ok(Self {
            pool,
            version,
            caps,
        })
    }

    async fn detect_version(
        pool: &Pool<SqlServerConnectionManager>,
    ) -> Result<ServerVersion, String> {
        let mut client = pool.get().await.map_err(|e| e.to_string())?;

        let rows = client
            .simple_query("SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128))")
            .await
            .map_err(|e| e.to_string())?
            .into_results()
            .await
            .map_err(|e| e.to_string())?;

        let raw = rows
            .into_iter()
            .flatten()
            .next()
            .and_then(|row| row.get::<&str, _>(0).map(|s| s.to_string()))
            .unwrap_or_else(|| "0.0.0".into());

        Ok(ServerVersion::parse(&raw))
    }
}

#[async_trait]
impl DatabaseDriver for SqlServerDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();
        let mut client = self.pool.get().await.map_err(|e| e.to_string())?;

        let rows = client
            .simple_query(
                "SELECT CONCAT(
                    'Microsoft SQL Server ',
                    CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(50)),
                    ' (',
                    CAST(SERVERPROPERTY('Edition') AS NVARCHAR(100)),
                    ')'
                )",
            )
            .await
            .map_err(|e| e.to_string())?
            .into_results()
            .await
            .map_err(|e| e.to_string())?;

        let version_str = rows
            .into_iter()
            .flatten()
            .next()
            .and_then(|row| row.get::<&str, _>(0).map(|s| s.to_string()))
            .unwrap_or_else(|| self.version.raw.clone());

        Ok((version_str, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        // tiberius Client is not Clone or Send across await points in the pool
        // so we acquire once and pass a mutable reference through every fetch
        let mut client = self.pool.get().await.map_err(|e| e.to_string())?;

        let tables = schema::fetch_tables(&mut client, &self.caps).await?;
        let views = schema::fetch_views(&mut client, &self.caps).await?;
        let functions = schema::fetch_functions(&mut client, &self.caps).await?;
        let procedures = schema::fetch_procedures(&mut client, &self.caps).await?;
        let sequences = schema::fetch_sequences(&mut client, &self.caps).await?;
        let types = schema::fetch_types(&mut client, &self.caps).await?;
        let jobs = schema::fetch_agent_jobs(&mut client, &self.caps).await?;

        let schemas =
            schema::group_by_schema(tables, views, functions, procedures, sequences, types);

        Ok(DatabaseMetadata::Relational(SchemaInfo {
            database_name: format!(
                "{} ({})",
                SqlServerCapabilities::product_name(&self.version),
                self.version.raw,
            ),
            schemas,
            scheduled_jobs: jobs,
        }))
    }
}
