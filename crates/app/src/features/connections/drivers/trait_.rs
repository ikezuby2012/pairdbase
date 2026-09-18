use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mongodb::bson::{doc, Bson, Document};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use super::versions::{ServerVersion, VersionCapabilities};
// use bson::{doc, Bson, Document};

// ── Relational database schema ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintInfo {
    pub name: String,
    pub constraint_type: String, // CHECK, UNIQUE, NOT NULL
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_primary: bool,
    pub is_indexed: bool,
    pub is_unique: bool,
    pub default: Option<String>,
    pub max_length: Option<i64>,
    pub numeric_precision: Option<i64>,
    pub comment: Option<String>,
}

// ── Tables ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub row_estimate: Option<i64>,
    pub size_bytes: Option<i64>,
    pub comment: Option<String>,
    pub columns: Vec<ColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub foreign_keys: Vec<ForeignKey>,
    pub triggers: Vec<TriggerInfo>,
    pub constraints: Vec<ConstraintInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    pub index_type: String,        // BTREE, HASH, GIN, GIST, etc.
    pub condition: Option<String>, // partial index WHERE clause
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub name: String,
    pub column: String,
    pub ref_schema: String,
    pub ref_table: String,
    pub ref_column: String,
    pub on_delete: String, // CASCADE, SET NULL, RESTRICT, etc.
    pub on_update: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ViewInfo {
    pub schema: String,
    pub name: String,
    pub definition: String, // the SELECT statement
    pub is_updatable: bool,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TriggerInfo {
    pub schema: String,
    pub name: String,
    pub table_name: String,
    pub timing: String,      // BEFORE, AFTER, INSTEAD OF
    pub events: Vec<String>, // INSERT, UPDATE, DELETE, TRUNCATE
    pub definition: String,  // trigger body / function call
    pub enabled: bool,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FunctionInfo {
    pub schema: String,
    pub name: String,
    pub language: String, // plpgsql, sql, python, java, etc.
    pub return_type: String,
    pub arguments: Vec<ArgumentInfo>,
    pub definition: String, // full source
    pub is_aggregate: bool,
    pub is_window: bool,
    pub volatility: Option<String>, // VOLATILE, STABLE, IMMUTABLE (PostgreSQL)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArgumentInfo {
    pub name: Option<String>,
    pub data_type: String,
    pub mode: String, // IN, OUT, INOUT
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcedureInfo {
    pub schema: String,
    pub name: String,
    pub language: String,
    pub arguments: Vec<ArgumentInfo>,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SequenceInfo {
    pub schema: String,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub increment: i64,
    pub cycle: bool,
    pub last_value: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScheduledJobInfo {
    pub name: String,
    pub schema: Option<String>,
    pub enabled: bool,
    pub schedule: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,
    pub job_type: String,
    pub definition: String,
}

// ── Materialized Views (PostgreSQL / Oracle) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MaterializedViewInfo {
    pub schema: String,
    pub name: String,
    pub definition: String,
    pub last_refresh_at: Option<DateTime<Utc>>,
    pub auto_refresh: bool,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TypeInfo {
    pub schema: String,
    pub name: String,
    pub type_: String,
    pub values: Vec<String>,
    pub definition: Option<String>,
}

// ── Extensions (PostgreSQL) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: String,
    pub schema: String,
}

// ── Packages (Oracle) ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PackageInfo {
    pub schema: String,
    pub name: String,
    pub header: String, // package spec (interface)
    pub body: String,   // package body (implementation)
    pub status: String, // VALID, INVALID
}

// ── Schema-level catalogue ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaCatalogue {
    pub name: String,
    pub tables: Vec<TableInfo>,
    pub views: Vec<ViewInfo>,
    pub materialized_views: Vec<MaterializedViewInfo>,
    pub functions: Vec<FunctionInfo>,
    pub procedures: Vec<ProcedureInfo>,
    pub triggers: Vec<TriggerInfo>,
    pub sequences: Vec<SequenceInfo>,
    pub types: Vec<TypeInfo>,           // PostgreSQL / Oracle
    pub extensions: Vec<ExtensionInfo>, // PostgreSQL
    pub packages: Vec<PackageInfo>,     // Oracle
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaInfo {
    pub database_name: String,
    pub schemas: Vec<SchemaCatalogue>,
    pub scheduled_jobs: Vec<ScheduledJobInfo>, // DB-level (not schema-scoped)
}

// ── Non-Relational (Mongodb) database schema ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct MongoDatabaseStats {
    pub collections: Option<i64>,
    pub views: Option<i64>,
    pub objects: Option<i64>,
    pub data_size_bytes: Option<i64>,
    pub storage_size_bytes: Option<i64>,
    pub indexes: Option<i64>,
    pub index_size_bytes: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MongoSchemaInfo {
    pub database: MongoDatabaseInfo,
    pub collections: Vec<MongoCollectionInfo>,
    pub views: Vec<MongoViewInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, JsonSchema)]
pub struct MongoDatabaseInfo {
    pub name: String,
    pub stats: MongoDatabaseStats,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoCollectionInfo {
    pub name: String,
    pub collection_type: MongoCollectionType,
    pub options: MongoCollectionOptions,
    pub stats: Option<MongoCollectionStats>,
    pub fields: Vec<MongoFieldInfo>,
    pub indexes: Vec<MongoIndexInfo>,
    pub validator: Option<MongoValidator>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MongoCollectionOptions {
    pub capped: bool,
    pub max_size_bytes: Option<i64>,
    pub max_documents: Option<i64>,
    pub timeseries: Option<MongoTimeSeriesOptions>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoTimeSeriesOptions {
    pub time_field: String,
    pub meta_field: Option<String>,
    pub granularity: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoCollectionStats {
    pub document_count: Option<i64>,
    pub size_bytes: Option<i64>,
    pub storage_size_bytes: Option<i64>,
    pub total_index_size_bytes: Option<i64>,
    pub index_count: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoFieldInfo {
    pub name: String,
    pub bson_types: Vec<String>,
    pub nullable: bool,
    pub optional: bool,
    pub occurrences: u64,
    pub sample_value: Option<String>,
    pub nested: Vec<MongoFieldInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoIndexInfo {
    pub name: String,
    pub keys: Vec<MongoIndexKey>,
    pub unique: bool,
    pub sparse: bool,
    pub partial: bool,
    pub ttl_seconds: Option<i64>,
    pub hidden: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MongoCollectionType {
    Collection,
    View,
    TimeSeries,
    Capped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MongoIndexOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoViewInfo {
    pub name: String,
    pub source: String,
    pub pipeline: Vec<Document>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoIndexKey {
    pub field: String,
    pub order: MongoIndexOrder,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MongoValidator {
    pub expression: Option<Document>,
    pub validation_level: Option<String>,
    pub validation_action: Option<String>,
}

// Redis-specific schema types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisSchemaInfo {
    pub server_info: RedisServerInfo,
    pub key_sample:  Vec<RedisKeyInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisServerInfo {
    pub version:             String,
    pub mode:                String,      // standalone, cluster, sentinel
    pub os:                  String,
    pub uptime_secs:         u64,
    pub used_memory:         String,
    pub max_memory:          String,
    pub role:                String,      // master, replica
    pub connected_replicas:  u32,
    pub databases:           std::collections::HashMap<u32, u64>,  // db_index → key_count
    pub acl_users:           Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisKeyInfo {
    pub key:      String,
    pub key_type: RedisKeyType,
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedisKeyType {
    String,
    Hash,
    List,
    Set,
    ZSet,
    Stream,
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DatabaseMetadata {
    Relational(SchemaInfo),
    Mongo(MongoSchemaInfo),
    Redis(RedisSchemaInfo), 
}

#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    fn version(&self) -> &ServerVersion;
    // fn capabilities(&self) -> &VersionCapabilities;
    async fn ping(&self) -> Result<(String, u64), String>;
    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String>;
}
