use futures::future::BoxFuture;
use sqlx::mysql::MySqlRow;
use sqlx::{AssertSqlSafe, MySqlPool, Row};

use super::queries::MySqlQueries;
use crate::features::connections::drivers::{
    trait_::{
        ArgumentInfo, ColumnInfo, ConstraintInfo, ExtensionInfo, ForeignKey, FunctionInfo,
        IndexInfo, MaterializedViewInfo, PackageInfo, ProcedureInfo, ScheduledJobInfo,
        SchemaCatalogue, SchemaInfo, SequenceInfo, TableInfo, TriggerInfo, TypeInfo, ViewInfo,
    },
    versions::VersionCapabilities,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn str_val(row: &MySqlRow, col: &str) -> String {
    row.try_get::<String, _>(col).unwrap_or_default()
}

fn opt_str(row: &MySqlRow, col: &str) -> Option<String> {
    row.try_get::<String, _>(col).ok().filter(|s| !s.is_empty())
}

fn bool_from_str(row: &MySqlRow, col: &str, true_val: &str) -> bool {
    row.try_get::<String, _>(col)
        .map(|s| s == true_val)
        .unwrap_or(false)
}

fn i64_val(row: &MySqlRow, col: &str) -> i64 {
    row.try_get::<i64, _>(col).unwrap_or(0)
}

fn run_query(pool: &MySqlPool, sql: &str) -> BoxFuture<'static, Result<Vec<MySqlRow>, String>> {
    let pool = pool.clone();
    let sql = sql.to_owned();

    Box::pin(async move {
        sqlx::query(AssertSqlSafe(sql))
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())
    })
}

// ── Tables ────────────────────────────────────────────────────────────────────

pub async fn fetch_tables(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = MySqlQueries::new(caps);

    let table_rows = run_query(pool, q.list_tables()).await?;

    let mut tables: Vec<TableInfo> = table_rows
        .iter()
        .map(|row| TableInfo {
            schema: "".into(), // MySQL has no schema layer — database is the schema
            name: str_val(row, "table_name"),
            comment: opt_str(row, "table_comment"),
            row_estimate: row.try_get::<i64, _>("row_estimate").ok(),
            size_bytes: row.try_get::<i64, _>("size_bytes").ok(),
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            triggers: vec![],
            constraints: vec![],
        })
        .collect();

    fetch_columns(pool, caps, &mut tables).await?;
    fetch_indexes(pool, caps, &mut tables).await?;
    fetch_foreign_keys(pool, caps, &mut tables).await?;
    fetch_constraints(pool, caps, &mut tables).await?;
    fetch_triggers(pool, caps, &mut tables).await?;

    Ok(tables)
}

async fn fetch_columns(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_columns()).await?;

    for row in &rows {
        let tname = str_val(row, "TABLE_NAME");

        if let Some(t) = tables.iter_mut().find(|t| t.name == tname) {
            let col_key = str_val(row, "COLUMN_KEY");

            t.columns.push(ColumnInfo {
                name: str_val(row, "COLUMN_NAME"),
                data_type: str_val(row, "DATA_TYPE"),
                nullable: bool_from_str(row, "IS_NULLABLE", "YES"),
                default: opt_str(row, "COLUMN_DEFAULT"),
                max_length: row.try_get::<i64, _>("max_length").ok(),
                numeric_precision: row.try_get::<i64, _>("NUMERIC_PRECISION").ok(),
                comment: opt_str(row, "COLUMN_COMMENT"),
                is_primary: col_key == "PRI",
                is_indexed: !col_key.is_empty(),
                is_unique: col_key == "UNI",
            });
        }
    }

    Ok(())
}

async fn fetch_indexes(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_indexes()).await?;

    for row in &rows {
        let tname = str_val(row, "TABLE_NAME");

        if let Some(t) = tables.iter_mut().find(|t| t.name == tname) {
            let index_name = str_val(row, "INDEX_NAME");
            let non_unique = row.try_get::<i64, _>("NON_UNIQUE").unwrap_or(1);

            let col_names: Vec<String> = str_val(row, "columns")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            t.indexes.push(IndexInfo {
                primary: index_name == "PRIMARY",
                unique: non_unique == 0,
                name: index_name,
                index_type: str_val(row, "INDEX_TYPE"),
                columns: col_names,
                condition: None,
            });
        }
    }

    Ok(())
}

async fn fetch_foreign_keys(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_foreign_keys()).await?;

    for row in &rows {
        let tname = str_val(row, "TABLE_NAME");

        if let Some(t) = tables.iter_mut().find(|t| t.name == tname) {
            t.foreign_keys.push(ForeignKey {
                name: str_val(row, "CONSTRAINT_NAME"),
                column: str_val(row, "COLUMN_NAME"),
                ref_schema: str_val(row, "ref_schema"),
                ref_table: str_val(row, "ref_table"),
                ref_column: str_val(row, "ref_col"),
                on_delete: str_val(row, "DELETE_RULE"),
                on_update: str_val(row, "UPDATE_RULE"),
            });
        }
    }

    Ok(())
}

async fn fetch_constraints(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_constraints()).await?;

    for row in &rows {
        let tname = str_val(row, "TABLE_NAME");

        if let Some(t) = tables.iter_mut().find(|t| t.name == tname) {
            t.constraints.push(ConstraintInfo {
                name: str_val(row, "CONSTRAINT_NAME"),
                constraint_type: str_val(row, "CONSTRAINT_TYPE"),
                definition: "".into(), // MySQL doesn't expose CHECK body via info schema
            });
        }
    }

    Ok(())
}

async fn fetch_triggers(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_triggers()).await?;

    for row in &rows {
        let tname = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut().find(|t| t.name == tname) {
            t.triggers.push(TriggerInfo {
                schema: "".into(),
                name: str_val(row, "TRIGGER_NAME"),
                table_name: tname,
                timing: str_val(row, "timing"),
                events: vec![str_val(row, "event")],
                definition: str_val(row, "definition"),
                enabled: true, // MySQL has no disable trigger — all are enabled
                language: "sql".into(),
            });
        }
    }

    Ok(())
}

// ── Views ─────────────────────────────────────────────────────────────────────

pub async fn fetch_views(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ViewInfo>, String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_views()).await?;

    Ok(rows
        .iter()
        .map(|row| ViewInfo {
            schema: "".into(),
            name: str_val(row, "view_name"),
            definition: str_val(row, "definition"),
            is_updatable: bool_from_str(row, "IS_UPDATABLE", "YES"),
            columns: vec![],
        })
        .collect())
}

// ── Functions ─────────────────────────────────────────────────────────────────

pub async fn fetch_functions(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<FunctionInfo>, String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_functions()).await?;

    Ok(rows
        .iter()
        .map(|row| FunctionInfo {
            schema: "".into(),
            name: str_val(row, "fn_name"),
            language: "sql".into(),
            return_type: str_val(row, "return_type"),
            definition: str_val(row, "definition"),
            is_aggregate: false,
            is_window: false,
            volatility: None,
            arguments: vec![],
        })
        .collect())
}

// ── Procedures ────────────────────────────────────────────────────────────────

pub async fn fetch_procedures(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ProcedureInfo>, String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_procedures()).await?;

    let mut procs: Vec<ProcedureInfo> = rows
        .iter()
        .map(|row| ProcedureInfo {
            schema: "".into(),
            name: str_val(row, "proc_name"),
            language: "sql".into(),
            definition: str_val(row, "definition"),
            arguments: vec![],
        })
        .collect();

    let param_rows = run_query(pool, q.list_procedure_params()).await?;

    for row in &param_rows {
        let pname = str_val(row, "proc_name");

        if let Some(proc) = procs.iter_mut().find(|p| p.name == pname) {
            let param_name = opt_str(row, "param_name");
            // Skip the return parameter (ORDINAL_POSITION = 0)
            if param_name.is_some() {
                proc.arguments.push(ArgumentInfo {
                    name: param_name,
                    data_type: str_val(row, "DATA_TYPE"),
                    mode: str_val(row, "mode"),
                    default: None,
                });
            }
        }
    }

    Ok(procs)
}

// ── Sequences (MariaDB 10.3+ only) ───────────────────────────────────────────

pub async fn fetch_sequences(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<SequenceInfo>, String> {
    let q = MySqlQueries::new(caps);

    let sql = match q.list_sequences() {
        Some(s) => s,
        None => {
            tracing::debug!("sequences not supported on this MySQL/MariaDB version — skipping");
            return Ok(vec![]);
        }
    };

    let rows = run_query(pool, sql).await?;

    Ok(rows
        .iter()
        .map(|row| SequenceInfo {
            schema: "".into(),
            name: str_val(row, "SEQUENCE_NAME"),
            data_type: "BIGINT".into(),
            start_value: i64_val(row, "START_VALUE"),
            min_value: i64_val(row, "MINIMUM_VALUE"),
            max_value: i64_val(row, "MAXIMUM_VALUE"),
            increment: i64_val(row, "INCREMENT"),
            cycle: bool_from_str(row, "CYCLE_OPTION", "YES"),
            last_value: None,
        })
        .collect())
}

// ── Scheduled events ──────────────────────────────────────────────────────────

pub async fn fetch_events(
    pool: &MySqlPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ScheduledJobInfo>, String> {
    let q = MySqlQueries::new(caps);
    let rows = run_query(pool, q.list_events()).await?;

    Ok(rows
        .iter()
        .map(|row| {
            let interval_val = opt_str(row, "INTERVAL_VALUE");
            let interval_field = opt_str(row, "INTERVAL_FIELD");

            let schedule = match (interval_val, interval_field) {
                (Some(v), Some(f)) => format!("EVERY {} {}", v, f),
                _ => "ONE TIME".into(),
            };

            ScheduledJobInfo {
                name: str_val(row, "EVENT_NAME"),
                schema: None,
                enabled: bool_from_str(row, "STATUS", "ENABLED"),
                schedule,
                definition: str_val(row, "definition"),
                last_status: None,
                last_run_at: None,
                next_run_at: None,
                job_type: "mysql_event".into(),
            }
        })
        .collect())
}

// ── Group into SchemaCatalogues ───────────────────────────────────────────────

pub fn group_by_schema(
    tables: Vec<TableInfo>,
    views: Vec<ViewInfo>,
    functions: Vec<FunctionInfo>,
    procedures: Vec<ProcedureInfo>,
    sequences: Vec<SequenceInfo>,
) -> Vec<SchemaCatalogue> {
    // MySQL has no schema layer — everything lives in one catalogue
    vec![SchemaCatalogue {
        name: "default".into(),
        tables,
        views,
        functions,
        procedures,
        sequences,
        materialized_views: vec![],
        triggers: vec![], // attached to tables
        types: vec![],
        extensions: vec![],
        packages: vec![],
    }]
}


