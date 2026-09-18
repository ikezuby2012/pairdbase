use std::str::FromStr;
use schemars::JsonSchema;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared::ConnectionId;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::features::connections::{connection_error::ConnectionError, vault::Vault};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DbType {
    PostgreSQL,
    MySQL,
    MongoDB,
    Redis,
    SQLite,
    SqlServer,
    Oracle
}

impl DbType  {
    pub fn as_str(&self) -> &'static str {
        match self {
            DbType::PostgreSQL => "postgresql",
            DbType::MySQL      => "mysql",
            DbType::MongoDB    => "mongodb",
            DbType::Redis      => "redis",
            DbType::SQLite     => "sqlite",
            DbType::SqlServer => "sqlserver",
            DbType::Oracle     => "oracle",
        }
    }

    pub fn default_port(&self) -> Option<u16> {
        match self {
            DbType::PostgreSQL => Some(5432),
            DbType::MySQL      => Some(3306),
            DbType::MongoDB    => Some(27017),
            DbType::Redis      => Some(6379),
            DbType::Oracle     => Some(1521),
            DbType::SqlServer  => Some(1433),
            DbType::SQLite     => None,
        }
    }
}

impl FromStr for DbType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "postgresql" | "postgres" => Ok(Self::PostgreSQL),
            "mysql" => Ok(Self::MySQL),
            "mssql" | "sqlserver" | "sql_server" => Ok(Self::SqlServer),
            "oracle" => Ok(Self::Oracle),
            "mongodb" | "mongo" => Ok(Self::MongoDB),
            other => Err(format!("Unsupported database type: {other}")),
        }
    }
}

impl TryFrom<&str> for DbType {
    type Error = ConnectionError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "postgresql" => Ok(DbType::PostgreSQL),
            "mysql"      => Ok(DbType::MySQL),
            "mongodb"    => Ok(DbType::MongoDB),
            "redis"      => Ok(DbType::Redis),
            "sqlite"     => Ok(DbType::SQLite),
            "sqlserver"  => Ok(DbType::SqlServer),
            "oracle"     => Ok(DbType::Oracle),
            other => Err(ConnectionError::UnsupportedDb(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SslMode {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}


impl Default for SslMode {
    fn default() -> Self { Self::Prefer }
}

impl SslMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable    => "disable",
            SslMode::Allow      => "allow",
            SslMode::Prefer     => "prefer",
            SslMode::Require    => "require",
            SslMode::VerifyCa   => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }
}

impl TryFrom<&str> for SslMode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "allow" => Ok(Self::Allow),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" => Ok(Self::VerifyCa),
            "verify-full" => Ok(Self::VerifyFull),

            value => Err(format!("invalid SSL mode: {value}")),
        }
    }
}

#[derive(Debug, Clone, JsonSchema)]
pub struct Connection {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub workspace_id: Uuid,

    pub name: String,
    pub db_type: DbType,

    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,

    pub password: Option<String>,

    pub ssl_mode: String,

    pub ssl_ca_cert: Option<String>,
    pub ssl_client_cert: Option<String>,
    pub ssl_client_key: Option<String>,

    // SSH
    pub ssh_enabled: bool,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<u16>,
    pub ssh_user: Option<String>,
    // Decrypted in-memory only
    pub ssh_private_key: Option<String>,

    pub read_only: bool,
    pub color: Option<String>,

    pub pool_min: u32,
    pub pool_max: u32,

    // Audit
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,
    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,

    // database versions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshConfig {
    pub host:        String,
    pub port:        u16,
    pub username:    String,
    pub private_key: String,   // plaintext PEM in-memory only
}

#[derive(Debug, Clone, Serialize, JsonSchema)] //JsonSchema
pub struct ConnectionView {
    pub id: ConnectionId,
    pub workspace_id: Uuid,
    pub name: String,
    pub db_type: String,

    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,

    pub ssl_mode: SslMode,
    pub ssh_enabled: bool,

    pub read_only: bool,
    pub color: Option<String>,

    pub created_at: DateTime<Utc>,
}

impl From<&Connection> for ConnectionView {
    fn from(c: &Connection) -> Self {
        Self {
            id:            ConnectionId(c.id),
            workspace_id:  c.workspace_id,
            name:          c.name.clone(),
            db_type:       c.db_type.as_str().to_string(),
            host:          c.host.clone(),
            port:          c.port,
            database_name: c.database_name.clone(),
            username:      c.username.clone(),
            ssl_mode:  match c.ssl_mode.as_str() {
                "prefer" => SslMode::Prefer,
                "allow" => SslMode::Allow,
                "require" => SslMode::Require,
                "verify-ca" => SslMode::VerifyCa,
                "verify-full" => SslMode::VerifyFull,
                _ => SslMode::Disable,
            },
            ssh_enabled:   c.ssh_enabled,
            read_only:     c.read_only,
            color:         c.color.clone(),
            created_at:    c.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ConnectionRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub workspace_id: Uuid,

    pub name: String,
    pub db_type: String,

    pub host: Option<String>,
    pub port: Option<i32>,
    pub database_name: Option<String>,
    pub username: Option<String>,

    pub password: Option<Vec<u8>>,

    pub ssl_mode: String,
    pub ssl_ca_cert: Option<Vec<u8>>,
    pub ssl_client_cert: Option<Vec<u8>>,
    pub ssl_client_key: Option<Vec<u8>>,

    pub ssh_enabled: bool,
    pub ssh_host: Option<String>,
    pub ssh_port: Option<i32>,
    pub ssh_user: Option<String>,
    pub ssh_private_key: Option<Vec<u8>>,

    pub read_only: bool,
    pub color: Option<String>,

    pub pool_min: i32,
    pub pool_max: i32,

    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,

    pub updated_by: Option<Uuid>,
    pub updated_at: Option<DateTime<Utc>>,

    pub is_soft_deleted: bool,
    pub deleted_by: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TestResult {
    pub success:        bool,
    pub message:        String,
    pub latency_ms:     Option<u64>,
    pub server_version: Option<String>,
}

impl ConnectionRow {
    pub fn into_domain(
        self,
        vault: &Vault,
    ) -> Result<Connection, ConnectionError> {
        Ok(Connection {
            id: self.id,
            organization_id: self.organization_id,
            workspace_id: self.workspace_id,

            name: self.name,

            db_type: DbType::from_str(&self.db_type)
                .map_err(|e| ConnectionError::InvalidDbType(e.to_string()))?,

            host: self.host,
            port: self.port.map(|p| p as u16),
            database_name: self.database_name,
            username: self.username,

            password: vault
                .decrypt_opt(self.password.as_deref())
                .map_err(ConnectionError::Vault)?,

            ssl_mode: self.ssl_mode,

            ssl_ca_cert: vault
                .decrypt_opt(self.ssl_ca_cert.as_deref())
                .map_err(ConnectionError::Vault)?,

            ssl_client_cert: vault
                .decrypt_opt(self.ssl_client_cert.as_deref())
                .map_err(ConnectionError::Vault)?,

            ssl_client_key: vault
                .decrypt_opt(self.ssl_client_key.as_deref())
                .map_err(ConnectionError::Vault)?,

            ssh_enabled: self.ssh_enabled,
            ssh_host: self.ssh_host,
            ssh_port: self.ssh_port.map(|p| p as u16),
            ssh_user: self.ssh_user,

            ssh_private_key: vault
                .decrypt_opt(self.ssh_private_key.as_deref())
                .map_err(ConnectionError::Vault)?,

            read_only: self.read_only,
            color: self.color,

            pool_min: self.pool_min as u32,
            pool_max: self.pool_max as u32,

            created_by: self.created_by,
            created_at: self.created_at,

            updated_by: self.updated_by,
            updated_at: self.updated_at,

            is_soft_deleted: self.is_soft_deleted,
            deleted_by: self.deleted_by,
            deleted_at: self.deleted_at,
        })
    }
}