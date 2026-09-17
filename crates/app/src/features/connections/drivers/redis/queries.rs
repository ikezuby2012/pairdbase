use crate::features::connections::drivers::versions::VersionCapabilities;

/// Redis has no SQL dialect and no schema queries.
/// This file holds the Redis command strings used for
/// metadata inspection (keyspace, key types, server info).
pub struct RedisQueries<'a> {
    #[allow(dead_code)]
    caps: &'a VersionCapabilities,
}

impl<'a> RedisQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    /// Returns server version, memory usage, connected clients
    pub fn server_info(&self) -> &'static str {
        "INFO server"
    }

    /// Returns keyspace stats — which databases exist and how many keys
    pub fn keyspace_info(&self) -> &'static str {
        "INFO keyspace"
    }

    /// Returns memory stats
    pub fn memory_info(&self) -> &'static str {
        "INFO memory"
    }

    /// Returns replication info — role, master/replica status
    pub fn replication_info(&self) -> &'static str {
        "INFO replication"
    }

    /// Scan keys safely — never use KEYS * in production
    /// Returns cursor + batch of keys
    pub fn scan(&self, cursor: u64, pattern: &str, count: u64) -> String {
        format!("SCAN {} MATCH {} COUNT {}", cursor, pattern, count)
    }

    /// Get type of a key
    pub fn key_type(&self, key: &str) -> String {
        format!("TYPE {}", key)
    }

    /// Get TTL of a key in seconds (-1 = no expiry, -2 = does not exist)
    pub fn ttl(&self, key: &str) -> String {
        format!("TTL {}", key)
    }

    /// Get all fields and values of a hash
    pub fn hgetall(&self, key: &str) -> String {
        format!("HGETALL {}", key)
    }

    /// Get all members of a set
    pub fn smembers(&self, key: &str) -> String {
        format!("SMEMBERS {}", key)
    }

    /// Get all members of a sorted set with scores
    pub fn zrange_with_scores(&self, key: &str) -> String {
        format!("ZRANGE {} 0 -1 WITHSCORES", key)
    }

    /// Get all elements of a list
    pub fn lrange(&self, key: &str) -> String {
        format!("LRANGE {} 0 -1", key)
    }

    /// Get string value
    pub fn get(&self, key: &str) -> String {
        format!("GET {}", key)
    }

    /// List all ACL users (Redis 6+)
    pub fn acl_users(&self) -> Option<&'static str> {
        if self.caps.redis_acl {
            Some("ACL LIST")
        } else {
            None
        }
    }
}