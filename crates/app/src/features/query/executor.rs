use mongodb::bson;
use serde_json::Value;
use sqlx::AssertSqlSafe;
use std::sync::Arc;
use std::time::Instant;
use tokio::task;
use uuid::Uuid;

use super::{
    domain::ColumnMeta,
    error::QueryError,
    pool::{ExecutionPool, ExecutionPoolRegistry},
};
use crate::features::connections::domain::DbType;

pub struct RawResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Value>>,
    pub truncated: bool,
    pub rows_affected: u64,
}

pub struct QueryExecutor {
    pools: Arc<ExecutionPoolRegistry>,
}

impl QueryExecutor {
    pub fn new(pools: Arc<ExecutionPoolRegistry>) -> Self {
        Self { pools }
    }

    pub async fn run(
        &self,
        connection_id: Uuid,
        db_type: &DbType,
        query: &str,
        max_rows: usize,
        timeout_secs: u64,
    ) -> Result<RawResult, QueryError> {
        let pool = self
            .pools
            .get(connection_id)
            .ok_or(QueryError::ConnectionNotFound)?;

        tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            self.dispatch(&pool, db_type, query, max_rows),
        )
        .await
        .map_err(|_| QueryError::Timeout(timeout_secs))?
    }

    async fn dispatch(
        &self,
        pool: &ExecutionPool,
        db_type: &DbType,
        query: &str,
        max_rows: usize,
    ) -> Result<RawResult, QueryError> {
        match pool {
            ExecutionPool::Postgres(p) => run_postgres(p, query, max_rows).await,
            ExecutionPool::MySQL(p) => run_mysql(p, query, max_rows).await,
            ExecutionPool::SQLite(p) => run_sqlite(p, query, max_rows).await,
            ExecutionPool::Oracle(p) => run_oracle(p, query, max_rows).await,
            ExecutionPool::SQLServer(p) => run_sqlserver(p, query, max_rows).await,
            ExecutionPool::MongoDB(c) => run_mongodb(c, db_type, query, max_rows).await,
            ExecutionPool::Redis(c) => run_redis(c, query).await,
        }
    }

    /// Ping the underlying pool — used to verify connection is alive
    /// before executing a query
    pub async fn ping(&self, connection_id: Uuid) -> Result<u64, String> {
        let pool = self
            .pools
            .get(connection_id)
            .ok_or_else(|| "no pool".to_string())?;

        ping_pool(&pool).await
    }
}

pub async fn ping_pool(pool: &ExecutionPool) -> Result<u64, String> {
    let start = Instant::now();

    match pool {
        ExecutionPool::Postgres(p) => {
            sqlx::query("SELECT 1")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
        }

        ExecutionPool::MySQL(p) => {
            sqlx::query("SELECT 1")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
        }

        ExecutionPool::SQLite(p) => {
            sqlx::query("SELECT 1")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
        }

        ExecutionPool::Oracle(p) => {
            let pool = Arc::clone(p);
            task::spawn_blocking(move || {
                let conn = pool.get().map_err(|e| e.to_string())?;
                conn.query_row("SELECT 1 FROM DUAL", &[])
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }

        ExecutionPool::SQLServer(p) => {
            let mut client = p.get().await.map_err(|e| e.to_string())?;
            client
                .simple_query("SELECT 1")
                .await
                .map_err(|e| e.to_string())?
                .into_results()
                .await
                .map_err(|e| e.to_string())?;
        }

        ExecutionPool::MongoDB(c) => {
            c.database("admin")
                .run_command(bson::doc! { "ping": 1 })
                .await
                .map_err(|e| e.to_string())?;
        }

        ExecutionPool::Redis(c) => {
            let mut conn = c
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| e.to_string())?;

            redis::cmd("PING")
                .query_async::<String>(&mut conn)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(start.elapsed().as_millis() as u64)
}

async fn run_postgres(
    pool: &sqlx::PgPool,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    use sqlx::{Column, Row, TypeInfo};

    let rows = sqlx::query(AssertSqlSafe(query))
        .fetch_all(pool)
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let truncated = rows.len() > max_rows;
    let rows_affected = rows.len() as u64;
    let rows = &rows[..rows.len().min(max_rows)];

    let columns: Vec<ColumnMeta> = rows
        .first()
        .map(|r| {
            r.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    data_type: c.type_info().name().to_string(),
                    nullable: true,
                })
                .collect()
        })
        .unwrap_or_default();

    let data = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| pg_value_to_json(row, i, col.type_info().name()))
                .collect()
        })
        .collect();

    Ok(RawResult {
        columns,
        rows: data,
        truncated,
        rows_affected,
    })
}

fn pg_value_to_json(row: &sqlx::postgres::PgRow, i: usize, type_name: &str) -> Value {
    use sqlx::Row;
    match type_name {
        "INT2" | "INT4" => row
            .try_get::<i32, _>(i)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        "INT8" => row
            .try_get::<i64, _>(i)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),
        "FLOAT4" | "FLOAT8" => row
            .try_get::<f64, _>(i)
            .ok()
            .and_then(|v| serde_json::Number::from_f64(v))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        "BOOL" => row
            .try_get::<bool, _>(i)
            .map(Value::Bool)
            .unwrap_or(Value::Null),
        "JSON" | "JSONB" => row.try_get::<Value, _>(i).unwrap_or(Value::Null),
        "TIMESTAMPTZ" | "TIMESTAMP" => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(i)
            .map(|v| Value::String(v.to_rfc3339()))
            .unwrap_or(Value::Null),
        "DATE" => row
            .try_get::<chrono::NaiveDate, _>(i)
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),
        "UUID" => row
            .try_get::<uuid::Uuid, _>(i)
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),
        "NUMERIC" => row
            .try_get::<sqlx::types::BigDecimal, _>(i)
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),
        _ => row
            .try_get::<String, _>(i)
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

async fn run_mysql(
    pool: &sqlx::MySqlPool,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    use sqlx::{Column, Row, TypeInfo};

    let rows = sqlx::query(AssertSqlSafe(query))
        .fetch_all(pool)
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let truncated = rows.len() > max_rows;
    let rows_affected = rows.len() as u64;
    let rows = &rows[..rows.len().min(max_rows)];

    let columns: Vec<ColumnMeta> = rows
        .first()
        .map(|r| {
            r.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    data_type: c.type_info().name().to_string(),
                    nullable: true,
                })
                .collect()
        })
        .unwrap_or_default();

    let data = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    row.try_get::<String, _>(i)
                        .map(Value::String)
                        .unwrap_or(Value::Null)
                })
                .collect()
        })
        .collect();

    Ok(RawResult {
        columns,
        rows: data,
        truncated,
        rows_affected,
    })
}

async fn run_sqlite(
    pool: &sqlx::SqlitePool,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    use sqlx::{Column, Row, TypeInfo};

    let rows = sqlx::query(AssertSqlSafe(query))
        .fetch_all(pool)
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let truncated = rows.len() > max_rows;
    let rows_affected = rows.len() as u64;
    let rows = &rows[..rows.len().min(max_rows)];

    let columns: Vec<ColumnMeta> = rows
        .first()
        .map(|r| {
            r.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    data_type: c.type_info().name().to_string(),
                    nullable: true,
                })
                .collect()
        })
        .unwrap_or_default();

    let data = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    row.try_get::<String, _>(i)
                        .map(Value::String)
                        .unwrap_or(Value::Null)
                })
                .collect()
        })
        .collect();

    Ok(RawResult {
        columns,
        rows: data,
        truncated,
        rows_affected,
    })
}

// ── Oracle ────────────────────────────────────────────────────────────────────
// oracle-rs is synchronous — every call goes through spawn_blocking

async fn run_oracle(
    pool: &Arc<oracle::pool::Pool>,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    let pool = Arc::clone(pool);
    let query = query.to_string();

    task::spawn_blocking(move || -> Result<RawResult, QueryError> {
        let conn = pool
            .get()
            .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

        // Detect statement type to decide how to execute
        let upper = query.trim().to_uppercase();
        let is_query =
            upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("(");

        if is_query {
            execute_oracle_query(&conn, &query, max_rows)
        } else {
            execute_oracle_dml(&conn, &query)
        }
    })
    .await
    .map_err(|e| QueryError::Internal(e.to_string()))?
}

fn execute_oracle_query(
    conn: &oracle::Connection,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    let mut cursor = conn
        .query(query, &[])
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let col_info = cursor.column_info().to_vec();

    let columns: Vec<ColumnMeta> = col_info
        .iter()
        .map(|c| ColumnMeta {
            name: c.name().to_string(),
            data_type: oracle_type_name(c.oracle_type()),
            nullable: true,
        })
        .collect();

    let mut rows = vec![];
    let mut truncated = false;

    for row_result in cursor.by_ref() {
        if rows.len() >= max_rows {
            truncated = true;
            break;
        }

        let row = row_result.map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

        let values: Vec<Value> = (0..col_info.len())
            .map(|i| oracle_col_to_json(&row, i, col_info[i].oracle_type()))
            .collect();

        rows.push(values);
    }

    let rows_affected = rows.len() as u64;

    Ok(RawResult {
        columns,
        rows,
        truncated,
        rows_affected,
    })
}

fn execute_oracle_dml(conn: &oracle::Connection, query: &str) -> Result<RawResult, QueryError> {
    let stmt = conn
        .execute(query, &[])
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let affected = stmt
        .row_count()
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    conn.commit()
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    Ok(RawResult {
        columns: vec![ColumnMeta {
            name: "rows_affected".into(),
            data_type: "INTEGER".into(),
            nullable: false,
        }],
        rows: vec![vec![Value::Number(affected.into())]],
        truncated: false,
        rows_affected: affected,
    })
}

/// Convert Oracle OracleType to a readable string
fn oracle_type_name(t: &oracle::sql_type::OracleType) -> String {
    use oracle::sql_type::OracleType;
    match t {
        OracleType::Varchar2(_) => "VARCHAR2".into(),
        OracleType::NVarchar2(_) => "NVARCHAR2".into(),
        OracleType::Char(_) => "CHAR".into(),
        OracleType::NChar(_) => "NCHAR".into(),
        OracleType::Number(_, _) => "NUMBER".into(),
        OracleType::Float(_) => "FLOAT".into(),
        OracleType::Date => "DATE".into(),
        OracleType::Timestamp(_) => "TIMESTAMP".into(),
        OracleType::TimestampTZ(_) => "TIMESTAMP WITH TIME ZONE".into(),
        OracleType::TimestampLTZ(_) => "TIMESTAMP WITH LOCAL TIME ZONE".into(),
        OracleType::CLOB => "CLOB".into(),
        OracleType::BLOB => "BLOB".into(),
        OracleType::Raw(_) => "RAW".into(),
        OracleType::Int64 => "INTEGER".into(),
        OracleType::UInt64 => "INTEGER UNSIGNED".into(),
        OracleType::Boolean => "BOOLEAN".into(),
        _ => "UNKNOWN".into(),
    }
}

/// Deserialise an Oracle row cell to serde_json::Value
fn oracle_col_to_json(
    row: &oracle::Row,
    idx: usize,
    type_: &oracle::sql_type::OracleType,
) -> Value {
    use oracle::sql_type::OracleType;

    match type_ {
        OracleType::Int64 | OracleType::UInt64 => row
            .get::<_, i64>(idx)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),

        OracleType::Number(_, _) | OracleType::Float(_) => {
            // Try integer first, then float, then string for large decimals
            if let Ok(v) = row.get::<_, i64>(idx) {
                Value::Number(v.into())
            } else if let Ok(v) = row.get::<_, f64>(idx) {
                serde_json::Number::from_f64(v)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                row.get::<_, String>(idx)
                    .map(Value::String)
                    .unwrap_or(Value::Null)
            }
        }

        OracleType::Boolean => row
            .get::<_, bool>(idx)
            .map(Value::Bool)
            .unwrap_or(Value::Null),

        OracleType::Date
        | OracleType::Timestamp(_)
        | OracleType::TimestampTZ(_)
        | OracleType::TimestampLTZ(_) => {
            // Oracle dates deserialise best as strings via Display
            row.get::<_, String>(idx)
                .map(Value::String)
                .unwrap_or(Value::Null)
        }

        OracleType::CLOB => {
            // CLOB: read as String (may be truncated for very large values)
            row.get::<_, String>(idx)
                .map(Value::String)
                .unwrap_or(Value::Null)
        }

        OracleType::BLOB => {
            // BLOB: return as base64 string
            row.get::<_, Vec<u8>>(idx)
                .map(|b| Value::String(base64_encode(&b)))
                .unwrap_or(Value::Null)
        }

        // Everything else — VARCHAR2, NVARCHAR2, CHAR, RAW, etc.
        _ => row
            .get::<_, String>(idx)
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(data)
}

// ── SQL Server ────────────────────────────────────────────────────────────────

async fn run_sqlserver(
    pool: &bb8::Pool<super::pool::SqlServerManager>,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    let mut client = pool
        .get()
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let upper = query.trim().to_uppercase();
    let is_query =
        upper.starts_with("SELECT") || upper.starts_with("WITH") || upper.starts_with("(");

    if is_query {
        execute_sqlserver_query(&mut client, query, max_rows).await
    } else {
        execute_sqlserver_dml(&mut client, query).await
    }
}

async fn execute_sqlserver_query(
    client: &mut tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    let results = client
        .simple_query(query)
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?
        .into_results()
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    // simple_query returns multiple result sets — take first
    let first = match results.into_iter().next() {
        Some(r) => r,
        None => {
            return Ok(RawResult {
                columns: vec![],
                rows: vec![],
                truncated: false,
                rows_affected: 0,
            })
        }
    };

    let truncated = first.len() > max_rows;
    let rows_affected = first.len() as u64;
    let slice = &first[..first.len().min(max_rows)];

    let columns: Vec<ColumnMeta> = slice
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|c| ColumnMeta {
                    name: c.name().to_string(),
                    data_type: format!("{:?}", c.column_type()),
                    nullable: true,
                })
                .collect()
        })
        .unwrap_or_default();

    let data = slice
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| tiberius_col_to_json(row, i, col))
                .collect()
        })
        .collect();

    Ok(RawResult {
        columns,
        rows: data,
        truncated,
        rows_affected,
    })
}

async fn execute_sqlserver_dml(
    client: &mut tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>,
    query: &str,
) -> Result<RawResult, QueryError> {
    let result = client
        .execute(query, &[])
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    let rows_affected = result.rows_affected().iter().sum::<u64>();

    Ok(RawResult {
        columns: vec![ColumnMeta {
            name: "rows_affected".into(),
            data_type: "BIGINT".into(),
            nullable: false,
        }],
        rows: vec![vec![Value::Number(rows_affected.into())]],
        truncated: false,
        rows_affected,
    })
}

/// Convert a tiberius column value to serde_json::Value
fn tiberius_col_to_json(row: &tiberius::Row, idx: usize, col: &tiberius::Column) -> Value {
    use tiberius::ColumnType;

    match col.column_type() {
        ColumnType::Bit => row
            .get::<bool, _>(idx)
            .map(Value::Bool)
            .unwrap_or(Value::Null),

        ColumnType::Int1 | ColumnType::Int2 | ColumnType::Int4 => row
            .get::<i32, _>(idx)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),

        ColumnType::Int8 => row
            .get::<i64, _>(idx)
            .map(|v| Value::Number(v.into()))
            .unwrap_or(Value::Null),

        ColumnType::Float4 => row
            .get::<f32, _>(idx)
            .and_then(|v| serde_json::Number::from_f64(v as f64).map(Value::Number))
            .unwrap_or(Value::Null),

        ColumnType::Float8 => row
            .get::<f64, _>(idx)
            .and_then(|v| serde_json::Number::from_f64(v).map(Value::Number))
            .unwrap_or(Value::Null),

        ColumnType::Datetime
        | ColumnType::Datetime2
        | ColumnType::Datetimen
        | ColumnType::DatetimeOffsetn => {
            // tiberius returns chrono types
            row.get::<chrono::NaiveDateTime, _>(idx)
                .map(|v| Value::String(v.to_string()))
                .unwrap_or_else(|| {
                    row.get::<&str, _>(idx)
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null)
                })
        }

        ColumnType::Decimaln | ColumnType::Numericn | ColumnType::Money | ColumnType::Money4 => {
            // Return as string to preserve precision
            row.get::<&str, _>(idx)
                .map(|s| Value::String(s.to_string()))
                .unwrap_or(Value::Null)
        }

        ColumnType::Guid => row
            .get::<uuid::Uuid, _>(idx)
            .map(|v| Value::String(v.to_string()))
            .unwrap_or(Value::Null),

        ColumnType::BigVarBin | ColumnType::BigBinary | ColumnType::Image => row
            .get::<&[u8], _>(idx)
            .map(|b| Value::String(base64_encode(b)))
            .unwrap_or(Value::Null),

        // NVarchar, Varchar, Char, NChar, Text, NText, Xml, etc.
        _ => row
            .get::<&str, _>(idx)
            .map(|s| Value::String(s.to_string()))
            .unwrap_or(Value::Null),
    }
}

// ── MongoDB ───────────────────────────────────────────────────────────────────

async fn run_mongodb(
    client: &mongodb::Client,
    _db_type: &DbType,
    query: &str,
    max_rows: usize,
) -> Result<RawResult, QueryError> {
    use futures::TryStreamExt;

    // Expect JSON: { "database": "...", "collection": "...", "pipeline": [...] }
    // or         : { "database": "...", "collection": "...", "filter": {...} }
    let doc: bson::Document = serde_json::from_str(query).map_err(|e| {
        QueryError::ExecutionFailed(format!("MongoDB query must be valid JSON: {}", e))
    })?;

    let db_name = doc.get_str("database").unwrap_or("test").to_string();

    let collection_name = doc
        .get_str("collection")
        .map_err(|_| {
            QueryError::ExecutionFailed("MongoDB query requires a 'collection' field".into())
        })?
        .to_string();

    let coll = client
        .database(&db_name)
        .collection::<bson::Document>(&collection_name);

    let mut cursor = if let Ok(pipeline) = doc.get_array("pipeline") {
        let stages: Vec<bson::Document> = pipeline
            .iter()
            .filter_map(|v| {
                if let bson::Bson::Document(d) = v {
                    Some(d.clone())
                } else {
                    None
                }
            })
            .collect();

        coll.aggregate(stages)
            .await
            .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?
    } else {
        let filter = doc.get_document("filter").cloned().unwrap_or_default();

        coll.find(filter)
            .await
            .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?
    };

    let mut docs = vec![];
    let mut truncated = false;

    while let Some(doc) = cursor
        .try_next()
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?
    {
        if docs.len() >= max_rows {
            truncated = true;
            break;
        }
        docs.push(doc);
    }

    let rows_affected = docs.len() as u64;

    let columns = vec![ColumnMeta {
        name: "document".into(),
        data_type: "bson".into(),
        nullable: false,
    }];

    let rows = docs
        .into_iter()
        .map(|d| vec![serde_json::to_value(&d).unwrap_or(Value::Null)])
        .collect();

    Ok(RawResult {
        columns,
        rows,
        truncated,
        rows_affected,
    })
}

// ── Redis ─────────────────────────────────────────────────────────────────────

async fn run_redis(client: &redis::Client, query: &str) -> Result<RawResult, QueryError> {
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    // Parse "COMMAND arg1 arg2 ..."
    let parts: Vec<&str> = query.trim().splitn(2, ' ').collect();
    let cmd_name = parts[0].to_uppercase();
    let args = parts.get(1).copied().unwrap_or("");

    let mut cmd = redis::cmd(&cmd_name);
    for arg in args.split_whitespace() {
        cmd.arg(arg);
    }

    let result: redis::Value = cmd
        .query_async(&mut conn)
        .await
        .map_err(|e| QueryError::ExecutionFailed(e.to_string()))?;

    Ok(RawResult {
        columns: vec![ColumnMeta {
            name: "result".into(),
            data_type: "redis_value".into(),
            nullable: true,
        }],
        rows: vec![vec![redis_value_to_json(result)]],
        truncated: false,
        rows_affected: 1,
    })
}

fn redis_value_to_json(val: redis::Value) -> Value {
    match val {
        redis::Value::Nil => Value::Null,
        redis::Value::Int(n) => Value::Number(n.into()),
        redis::Value::BulkString(bytes) => String::from_utf8(bytes)
            .map(Value::String)
            .unwrap_or(Value::Null),
        redis::Value::SimpleString(s) => Value::String(s),
        redis::Value::Okay => Value::String("OK".into()),
        redis::Value::Array(items) => {
            Value::Array(items.into_iter().map(redis_value_to_json).collect())
        }
        _ => Value::Null,
    }
}
