use async_trait::async_trait;
use std::time::Instant;

use crate::features::connections::{
    domain::Connection,
    drivers::{
        trait_::{DatabaseDriver, DatabaseMetadata, RedisSchemaInfo},
        versions::{ServerVersion, VersionCapabilities},
    },
};

mod queries;
mod schema;

pub struct RedisDriver {
    client: redis::Client,
    version: ServerVersion,
    caps: VersionCapabilities,
}

impl RedisDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let host = conn.host.as_deref().unwrap_or("localhost");
        let port = conn.port.unwrap_or(6379);

        let url = match &conn.password {
            Some(p) => format!("redis://:{p}@{host}:{port}"),
            None => format!("redis://{host}:{port}"),
        };

        let client = redis::Client::open(url).map_err(|e| e.to_string())?;

        // Verify connection + detect version
        let version = Self::detect_version(&client).await?;
        let caps = VersionCapabilities::for_redis(&version);

        tracing::info!(
            version     = %version.raw,
            has_acl     = caps.redis_acl,
            has_streams = caps.redis_streams,
            "Redis connected"
        );

        Ok(Self {
            client,
            version,
            caps,
        })
    }

    async fn detect_version(client: &redis::Client) -> Result<ServerVersion, String> {
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| e.to_string())?;

        let info: String = redis::cmd("INFO")
            .arg("server")
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;

        let raw = info
            .lines()
            .find(|l| l.starts_with("redis_version:"))
            .and_then(|l| l.splitn(2, ':').nth(1))
            .map(|v| v.trim().to_string())
            .unwrap_or_else(|| "0.0.0".into());

        Ok(ServerVersion::parse(&raw))
    }
}

#[async_trait]
impl DatabaseDriver for RedisDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }
    // fn capabilities(&self) -> &VersionCapabilities { &self.caps }

    async fn ping(&self) -> Result<(String, u64), String> {
        let start = Instant::now();
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| e.to_string())?;

        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| e.to_string())?;

        let version_str = format!("Redis {}", self.version.raw);
        Ok((version_str, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let server_info = schema::fetch_server_info(&self.client, &self.caps).await?;

        // Sample up to 100 keys from the default pattern
        let key_sample = schema::fetch_key_sample(&self.client, &self.caps, "*", 100).await?;

        Ok(DatabaseMetadata::Redis(RedisSchemaInfo {
            server_info,
            key_sample,
        }))
    }
}
