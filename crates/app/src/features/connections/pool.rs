use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::features::connections::drivers::trait_::DatabaseMetadata;

pub struct DriverRegistry {
    drivers: DashMap<Uuid, Arc<dyn crate::features::connections::drivers::trait_::DatabaseDriver>>,
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: DashMap::new(),
        }
    }

    pub fn register(
        &self,
        id: Uuid,
        driver: Arc<dyn crate::features::connections::drivers::trait_::DatabaseDriver>,
    ) {
        self.drivers.insert(id, driver);
    }

    pub fn get(
        &self,
        id: Uuid,
    ) -> Option<Arc<dyn crate::features::connections::drivers::trait_::DatabaseDriver>> {
        self.drivers.get(&id).map(|d| Arc::clone(d.value()))
    }

    pub fn remove(&self, id: Uuid) {
        self.drivers.remove(&id);
        tracing::debug!(connection_id = %id, "driver removed from registry");
    }

    pub fn contains(&self, id: Uuid) -> bool {
        self.drivers.contains_key(&id)
    }
}

pub struct SchemaCache {
    redis: redis::aio::ConnectionManager,
}

impl SchemaCache {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }

    fn key(connection_id: Uuid) -> String {
        format!("schema_cache:{}", connection_id)
    }

    /// Fetch cached schema — returns None on miss or deserialisation failure
    pub async fn get(&self, connection_id: Uuid) -> Option<DatabaseMetadata> {
        let mut conn = self.redis.clone();

        let raw: Option<String> = redis::cmd("GET")
            .arg(Self::key(connection_id))
            .query_async(&mut conn)
            .await
            .ok()?;

        raw.and_then(|s| {
            serde_json::from_str(&s)
                .map_err(|e| {
                    tracing::warn!(
                        connection_id = %connection_id,
                        error         = %e,
                        "schema cache deserialisation failed — treating as miss"
                    );
                })
                .ok()
        })
    }

    /// Store schema in Redis with 5-minute TTL
    pub async fn set(&self, connection_id: Uuid, metadata: &DatabaseMetadata) {
        let Ok(json) = serde_json::to_string(metadata) else {
            tracing::warn!(
                connection_id = %connection_id,
                "schema cache serialisation failed — skipping cache write"
            );
            return;
        };

        let mut conn = self.redis.clone();

        let result: Result<(), redis::RedisError> = redis::cmd("SETEX")
            .arg(Self::key(connection_id))
            .arg(300u64) // 5 minutes
            .arg(json)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => tracing::debug!(
                connection_id = %connection_id,
                "schema cached for 5 minutes"
            ),
            Err(e) => tracing::warn!(
                connection_id = %connection_id,
                error         = %e,
                "schema cache write failed — continuing without cache"
            ),
        }
    }

    /// Remove cached schema — called on connection update, delete, refresh
    pub async fn invalidate(&self, connection_id: Uuid) {
        let mut conn = self.redis.clone();

        let result: Result<(), redis::RedisError> = redis::cmd("DEL")
            .arg(Self::key(connection_id))
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => tracing::debug!(
                connection_id = %connection_id,
                "schema cache invalidated"
            ),
            Err(e) => tracing::warn!(
                connection_id = %connection_id,
                error         = %e,
                "schema cache invalidation failed"
            ),
        }
    }

    /// Invalidate all cached schemas for a list of connection IDs
    /// Used when a workspace is deleted
    pub async fn invalidate_many(&self, connection_ids: &[Uuid]) {
        if connection_ids.is_empty() {
            return;
        }

        let mut conn = self.redis.clone();

        let keys: Vec<String> = connection_ids.iter().map(|id| Self::key(*id)).collect();

        let result: Result<(), redis::RedisError> =
            redis::cmd("DEL").arg(keys).query_async(&mut conn).await;

        if let Err(e) = result {
            tracing::warn!(
                error = %e,
                count = connection_ids.len(),
                "bulk schema cache invalidation failed"
            );
        }
    }
}
