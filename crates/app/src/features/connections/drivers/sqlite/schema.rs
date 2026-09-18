use sqlx::{AssertSqlSafe, Row, SqlitePool};

use super::queries::SqliteQueries;
use crate::features::connections::drivers::{
    trait_::{
        ColumnInfo, ConstraintInfo, ForeignKey, IndexInfo, SchemaCatalogue, SchemaInfo, TableInfo,
        TriggerInfo, ViewInfo,
    },
    versions::VersionCapabilities,
};

// ── Tables ────────────────────────────────────────────────────────────────────

pub async fn fetch_tables(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = SqliteQueries::new(caps);

    let table_rows = sqlx::query(q.list_tables())
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut tables: Vec<TableInfo> = table_rows
        .iter()
        .map(|row| TableInfo {
            schema: "main".into(), // SQLite has one schema: main
            name: row.try_get::<String, _>("table_name").unwrap_or_default(),
            comment: None, // SQLite has no column comments
            row_estimate: None,
            size_bytes: None,
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            triggers: vec![],
            constraints: vec![],
        })
        .collect();

    // Fetch columns, indexes, foreign keys per table
    for table in &mut tables {
        table.columns = fetch_columns(pool, caps, &table.name).await?;
        table.indexes = fetch_indexes(pool, caps, &table.name).await?;
        table.foreign_keys = fetch_foreign_keys(pool, caps, &table.name).await?;
    }

    Ok(tables)
}

async fn fetch_columns(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
    table_name: &str,
) -> Result<Vec<ColumnInfo>, String> {
    let q = SqliteQueries::new(caps);
    let sql = q.table_info(table_name);

    // PRAGMA table_info returns: cid, name, type, notnull, dflt_value, pk
    let rows = sqlx::query(AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|row| {
            let pk: i64 = row.try_get("pk").unwrap_or(0);
            ColumnInfo {
                name: row.try_get::<String, _>("name").unwrap_or_default(),
                data_type: row
                    .try_get::<String, _>("type")
                    .unwrap_or_else(|_| "TEXT".into()),
                nullable: row.try_get::<i64, _>("notnull").unwrap_or(0) == 0,
                default: row.try_get::<String, _>("dflt_value").ok(),
                max_length: None,
                numeric_precision: None,
                comment: None,
                is_primary: pk > 0,
                is_indexed: pk > 0, // refined below when indexes are fetched
                is_unique: false,   // refined below
            }
        })
        .collect())
}

async fn fetch_indexes(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
    table_name: &str,
) -> Result<Vec<IndexInfo>, String> {
    let q = SqliteQueries::new(caps);

    // List all indexes for this table
    let index_rows = sqlx::query(q.list_indexes())
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let table_indexes: Vec<_> = index_rows
        .iter()
        .filter(|row| {
            row.try_get::<String, _>("table_name")
                .map(|t| t == table_name)
                .unwrap_or(false)
        })
        .collect();

    let mut indexes = vec![];

    for row in table_indexes {
        let index_name = row.try_get::<String, _>("index_name").unwrap_or_default();
        let ddl = row.try_get::<String, _>("ddl").unwrap_or_default();

        let is_unique = ddl.to_uppercase().contains("UNIQUE");

        // Get columns in this index
        let info_sql = q.index_info(&index_name);
        let info_rows = sqlx::query(AssertSqlSafe(info_sql))
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        let columns: Vec<String> = info_rows
            .iter()
            .map(|r| r.try_get::<String, _>("name").unwrap_or_default())
            .collect();

        indexes.push(IndexInfo {
            name: index_name.to_string(),
            unique: is_unique,
            primary: false, // SQLite PKs are implicit rowid, not named indexes
            index_type: "BTREE".into(),
            columns,
            condition: None,
        });
    }

    Ok(indexes)
}

async fn fetch_foreign_keys(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
    table_name: &str,
) -> Result<Vec<ForeignKey>, String> {
    let q = SqliteQueries::new(caps);
    let sql = q.foreign_keys(table_name);

    // PRAGMA foreign_key_list: id, seq, table, from, to, on_update, on_delete, match
    let rows = sqlx::query(AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|row| ForeignKey {
            name: format!(
                "fk_{}_{}",
                table_name,
                row.try_get::<i64, _>("id").unwrap_or(0)
            ),
            column: row.try_get::<String, _>("from").unwrap_or_default(),
            ref_schema: "main".into(),
            ref_table: row.try_get::<String, _>("table").unwrap_or_default(),
            ref_column: row.try_get::<String, _>("to").unwrap_or_default(),
            on_delete: row
                .try_get::<String, _>("on_delete")
                .unwrap_or_else(|_| "NO ACTION".into()),
            on_update: row
                .try_get::<String, _>("on_update")
                .unwrap_or_else(|_| "NO ACTION".into()),
        })
        .collect())
}

// ── Views ─────────────────────────────────────────────────────────────────────

pub async fn fetch_views(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
) -> Result<Vec<ViewInfo>, String> {
    let q = SqliteQueries::new(caps);
    let rows = sqlx::query(q.list_views())
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|row| ViewInfo {
            schema: "main".into(),
            name: row.try_get::<String, _>("view_name").unwrap_or_default(),
            definition: row.try_get::<String, _>("definition").unwrap_or_default(),
            is_updatable: false, // SQLite views are read-only
            columns: vec![],
        })
        .collect())
}

// ── Triggers ──────────────────────────────────────────────────────────────────

pub async fn fetch_triggers(
    pool: &SqlitePool,
    caps: &VersionCapabilities,
) -> Result<Vec<TriggerInfo>, String> {
    let q = SqliteQueries::new(caps);
    let rows = sqlx::query(q.list_triggers())
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|row| {
            let def = row.try_get::<String, _>("definition").unwrap_or_default();
            let upper = def.to_uppercase();

            let timing = if upper.contains("BEFORE") {
                "BEFORE"
            } else if upper.contains("INSTEAD") {
                "INSTEAD OF"
            } else {
                "AFTER"
            }
            .to_string();

            let mut events = vec![];
            if upper.contains("INSERT") {
                events.push("INSERT".into());
            }
            if upper.contains("UPDATE") {
                events.push("UPDATE".into());
            }
            if upper.contains("DELETE") {
                events.push("DELETE".into());
            }

            TriggerInfo {
                schema: "main".into(),
                name: row.try_get::<String, _>("trigger_name").unwrap_or_default(),
                table_name: row.try_get::<String, _>("table_name").unwrap_or_default(),
                timing,
                events,
                definition: def,
                enabled: true, // SQLite has no disable trigger
                language: "sql".into(),
            }
        })
        .collect())
}

// ── Group into SchemaCatalogue ────────────────────────────────────────────────

pub fn build_catalogue(
    tables: Vec<TableInfo>,
    views: Vec<ViewInfo>,
    triggers: Vec<TriggerInfo>,
) -> Vec<SchemaCatalogue> {
    // SQLite has one schema: main
    // Attached databases would be additional schemas — not supported yet
    vec![SchemaCatalogue {
        name: "main".into(),
        tables,
        views,
        triggers,
        functions: vec![],          // SQLite has no user-defined stored functions
        procedures: vec![],         // SQLite has no stored procedures
        sequences: vec![],          // SQLite uses AUTOINCREMENT, not sequences
        materialized_views: vec![], // SQLite has no materialized views
        types: vec![],              // SQLite is dynamically typed
        extensions: vec![],         // SQLite uses loadable extensions — not catalogued
        packages: vec![],           // SQLite has no packages
    }]
}
