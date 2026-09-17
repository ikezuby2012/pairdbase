use redis::AsyncCommands;
use std::collections::HashMap;

use crate::features::connections::drivers::{
    trait_::{
        DatabaseMetadata, RedisKeyInfo, RedisKeyType,
        RedisSchemaInfo, RedisServerInfo,
    },
    versions::VersionCapabilities,
};
use super::queries::RedisQueries;

// ── Redis metadata fetch ──────────────────────────────────────────────────────

pub async fn fetch_server_info(
    client: &redis::Client,
    caps:   &VersionCapabilities,
) -> Result<RedisServerInfo, String> {
    let q   = RedisQueries::new(caps);
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| e.to_string())?;

    // INFO server
    let server_raw: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut conn)
        .await
        .map_err(|e| e.to_string())?;

    // INFO keyspace
    let keyspace_raw: String = redis::cmd("INFO")
        .arg("keyspace")
        .query_async(&mut conn)
        .await
        .map_err(|e| e.to_string())?;

    // INFO memory
    let memory_raw: String = redis::cmd("INFO")
        .arg("memory")
        .query_async(&mut conn)
        .await
        .map_err(|e| e.to_string())?;

    // INFO replication
    let replication_raw: String = redis::cmd("INFO")
        .arg("replication")
        .query_async(&mut conn)
        .await
        .map_err(|e| e.to_string())?;

    let version        = parse_info_field(&server_raw, "redis_version")
        .unwrap_or_else(|| "unknown".into());
    let mode           = parse_info_field(&server_raw, "redis_mode")
        .unwrap_or_else(|| "standalone".into());
    let os             = parse_info_field(&server_raw, "os")
        .unwrap_or_default();
    let uptime_secs    = parse_info_field(&server_raw, "uptime_in_seconds")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let used_memory    = parse_info_field(&memory_raw, "used_memory_human")
        .unwrap_or_default();
    let max_memory     = parse_info_field(&memory_raw, "maxmemory_human")
        .unwrap_or_else(|| "unlimited".into());
    let role           = parse_info_field(&replication_raw, "role")
        .unwrap_or_else(|| "master".into());
    let connected_replicas = parse_info_field(&replication_raw, "connected_slaves")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    // Parse keyspace: db0:keys=100,expires=10,avg_ttl=0
    let databases = parse_keyspace(&keyspace_raw);

    // ACL users if supported
    let acl_users = match RedisQueries::new(caps).acl_users() {
        Some(_) => {
            let result: Result<Vec<String>, _> = redis::cmd("ACL")
                .arg("LIST")
                .query_async(&mut conn)
                .await;
            result.ok()
        }
        None => None,
    };

    Ok(RedisServerInfo {
        version,
        mode,
        os,
        uptime_secs,
        used_memory,
        max_memory,
        role,
        connected_replicas,
        databases,
        acl_users,
    })
}

pub async fn fetch_key_sample(
    client:  &redis::Client,
    caps:    &VersionCapabilities,
    pattern: &str,
    limit:   u64,
) -> Result<Vec<RedisKeyInfo>, String> {
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| e.to_string())?;

    let mut keys   = vec![];
    let mut cursor = 0u64;
    let batch      = limit.min(100);

    // SCAN safely — never KEYS *
    loop {
        let (next_cursor, batch_keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(batch)
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;

        for key in batch_keys {
            if keys.len() >= limit as usize {
                break;
            }

            let key_type_str: String = redis::cmd("TYPE")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap_or_else(|_| "unknown".into());

            let ttl: i64 = redis::cmd("TTL")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap_or(-1);

            let key_type = match key_type_str.as_str() {
                "string" => RedisKeyType::String,
                "hash"   => RedisKeyType::Hash,
                "list"   => RedisKeyType::List,
                "set"    => RedisKeyType::Set,
                "zset"   => RedisKeyType::ZSet,
                "stream" => RedisKeyType::Stream,
                _        => RedisKeyType::Unknown,
            };

            keys.push(RedisKeyInfo {
                key,
                key_type,
                ttl_secs: if ttl < 0 { None } else { Some(ttl as u64) },
            });
        }

        cursor = next_cursor;

        if cursor == 0 || keys.len() >= limit as usize {
            break;
        }
    }

    Ok(keys)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a field from Redis INFO output
/// INFO output format: "field_name:value\r\n"
fn parse_info_field(raw: &str, field: &str) -> Option<String> {
    raw.lines()
        .find(|line| line.starts_with(&format!("{}:", field)))
        .and_then(|line| line.splitn(2, ':').nth(1))
        .map(|v| v.trim().to_string())
}

/// Parse keyspace info into database index → key count map
/// Format: "db0:keys=100,expires=10,avg_ttl=0"
fn parse_keyspace(raw: &str) -> HashMap<u32, u64> {
    let mut map = HashMap::new();

    for line in raw.lines() {
        if !line.starts_with("db") { continue; }

        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 { continue; }

        let db_index: u32 = parts[0]
            .trim_start_matches("db")
            .parse()
            .unwrap_or(0);

        let key_count: u64 = parts[1]
            .split(',')
            .find(|s| s.starts_with("keys="))
            .and_then(|s| s.trim_start_matches("keys=").parse().ok())
            .unwrap_or(0);

        map.insert(db_index, key_count);
    }

    map
}