use tiberius::{Client, Row};
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use super::queries::SqlServerQueries;
use crate::features::connections::drivers::{
    trait_::{
        ArgumentInfo, ColumnInfo, ConstraintInfo, ForeignKey, FunctionInfo, IndexInfo,
        MaterializedViewInfo, PackageInfo, ProcedureInfo, ScheduledJobInfo, SchemaCatalogue,
        SchemaInfo, SequenceInfo, TableInfo, TriggerInfo, TypeInfo, ViewInfo,
    },
    versions::VersionCapabilities,
};

type SqlClient = Client<Compat<TcpStream>>;

fn str_val(row: &Row, idx: usize) -> String {
    row.get::<&str, _>(idx).unwrap_or("").to_string()
}

fn opt_str(row: &Row, idx: usize) -> Option<String> {
    row.get::<&str, _>(idx)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn bool_val(row: &Row, idx: usize) -> bool {
    row.get::<bool, _>(idx).unwrap_or(false)
}

fn i64_val(row: &Row, idx: usize) -> i64 {
    row.get::<i64, _>(idx).unwrap_or(0)
}

async fn run_query(client: &mut SqlClient, sql: &str) -> Result<Vec<Row>, String> {
    client
        .simple_query(sql)
        .await
        .map_err(|e| e.to_string())?
        .into_results()
        .await
        .map_err(|e| e.to_string())
        .map(|r| r.into_iter().flatten().collect())
}

pub async fn fetch_tables(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = SqlServerQueries::new(caps);

    let table_rows = run_query(client, q.list_tables()).await?;

    let mut tables: Vec<TableInfo> = table_rows
        .iter()
        .map(|row| TableInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            comment: opt_str(row, 2),
            row_estimate: Some(i64_val(row, 3)),
            size_bytes: Some(i64_val(row, 4)),
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            triggers: vec![],
            constraints: vec![],
        })
        .collect();

    fetch_columns(client, caps, &mut tables).await?;
    fetch_indexes(client, caps, &mut tables).await?;
    fetch_foreign_keys(client, caps, &mut tables).await?;
    fetch_constraints(client, caps, &mut tables).await?;
    fetch_triggers(client, caps, &mut tables).await?;

    Ok(tables)
}

async fn fetch_columns(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_columns()).await?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.columns.push(ColumnInfo {
                name: str_val(row, 2),
                data_type: str_val(row, 3),
                nullable: bool_val(row, 4),
                default: opt_str(row, 5),
                max_length: Some(i64_val(row, 6)),
                numeric_precision: Some(i64_val(row, 7)),
                comment: opt_str(row, 8),
                is_primary: false, // resolved via indexes
                is_indexed: false, // resolved via indexes
                is_unique: false,  // resolved via indexes
            });
        }
    }

    Ok(())
}

async fn fetch_indexes(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = SqlServerQueries::new(caps);
    let sql = q.list_indexes(); // returns String — version branched internally
    let rows = run_query(client, &sql).await?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let is_primary = bool_val(row, 4);
            let col_names: Vec<String> = str_val(row, 6)
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Mark columns as primary / indexed
            for col in &mut t.columns {
                if col_names.contains(&col.name) {
                    col.is_indexed = true;
                    if is_primary {
                        col.is_primary = true;
                    }
                    if bool_val(row, 3) {
                        col.is_unique = true;
                    }
                }
            }

            t.indexes.push(IndexInfo {
                name: str_val(row, 2),
                unique: bool_val(row, 3),
                primary: is_primary,
                index_type: str_val(row, 5),
                columns: col_names,
                condition: opt_str(row, 7),
            });
        }
    }

    Ok(())
}

async fn fetch_foreign_keys(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_foreign_keys()).await?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.foreign_keys.push(ForeignKey {
                name: str_val(row, 2),
                column: str_val(row, 3),
                ref_schema: str_val(row, 4),
                ref_table: str_val(row, 5),
                ref_column: str_val(row, 6),
                on_delete: str_val(row, 7),
                on_update: str_val(row, 8),
            });
        }
    }

    Ok(())
}

async fn fetch_constraints(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_constraints()).await?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.constraints.push(ConstraintInfo {
                name: str_val(row, 2),
                constraint_type: str_val(row, 3),
                definition: str_val(row, 4),
            });
        }
    }

    Ok(())
}

async fn fetch_triggers(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_triggers()).await?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let events: Vec<String> = str_val(row, 5)
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            t.triggers.push(TriggerInfo {
                schema: schema.clone(),
                name: str_val(row, 2),
                table_name: tname,
                enabled: !bool_val(row, 3),
                timing: str_val(row, 4),
                events,
                definition: str_val(row, 6),
                language: "T-SQL".into(),
            });
        }
    }

    Ok(())
}

// ── Views ─────────────────────────────────────────────────────────────────────

pub async fn fetch_views(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<ViewInfo>, String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_views()).await?;

    Ok(rows
        .iter()
        .map(|row| ViewInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            definition: str_val(row, 2),
            is_updatable: bool_val(row, 3),
            columns: vec![],
        })
        .collect())
}

// ── Functions ─────────────────────────────────────────────────────────────────

pub async fn fetch_functions(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<FunctionInfo>, String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_functions()).await?;

    Ok(rows
        .iter()
        .map(|row| FunctionInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            language: "T-SQL".into(),
            return_type: str_val(row, 2),
            definition: str_val(row, 3),
            is_aggregate: str_val(row, 4).contains("AGGREGATE"),
            is_window: false,
            volatility: None,
            arguments: vec![],
        })
        .collect())
}

pub async fn fetch_procedures(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<ProcedureInfo>, String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_procedures()).await?;

    let mut procs: Vec<ProcedureInfo> = rows
        .iter()
        .map(|row| ProcedureInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            language: "T-SQL".into(),
            definition: str_val(row, 2),
            arguments: vec![],
        })
        .collect();

    // Fetch parameters and attach to procedures
    let param_rows = run_query(client, q.list_procedure_params()).await?;

    for row in &param_rows {
        let schema = str_val(row, 0);
        let pname = str_val(row, 1);

        if let Some(proc) = procs
            .iter_mut()
            .find(|p| p.schema == schema && p.name == pname)
        {
            proc.arguments.push(ArgumentInfo {
                name: opt_str(row, 2),
                data_type: str_val(row, 3),
                mode: if bool_val(row, 4) {
                    "OUT".into()
                } else {
                    "IN".into()
                },
                default: opt_str(row, 6),
            });
        }
    }

    Ok(procs)
}

// ── Sequences ─────────────────────────────────────────────────────────────────

pub async fn fetch_sequences(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<SequenceInfo>, String> {
    let q = SqlServerQueries::new(caps);

    // Returns None on SQL Server < 2012 — skip entirely
    let sql = match q.list_sequences() {
        Some(s) => s,
        None => {
            tracing::debug!("sequences not supported on this SQL Server version — skipping");
            return Ok(vec![]);
        }
    };

    let rows = run_query(client, sql).await?;

    Ok(rows
        .iter()
        .map(|row| SequenceInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            data_type: str_val(row, 2),
            start_value: i64_val(row, 3),
            min_value: i64_val(row, 4),
            max_value: i64_val(row, 5),
            increment: i64_val(row, 6),
            cycle: bool_val(row, 7),
            last_value: Some(i64_val(row, 8)),
        })
        .collect())
}

pub async fn fetch_types(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<TypeInfo>, String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_types()).await?;

    Ok(rows
        .iter()
        .map(|row| TypeInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            type_: if bool_val(row, 3) {
                "table_type".into()
            } else {
                "scalar_type".into()
            },
            values: vec![],
            definition: Some(format!("Based on {}", str_val(row, 2))),
        })
        .collect())
}

// ── Temporal tables ───────────────────────────────────────────────────────────

pub async fn fetch_temporal_tables(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = SqlServerQueries::new(caps);

    let sql = match q.list_temporal_tables() {
        Some(s) => s,
        None => {
            tracing::debug!("temporal tables not supported on this SQL Server version — skipping");
            return Ok(vec![]);
        }
    };

    let rows = run_query(client, sql).await?;

    Ok(rows
        .iter()
        .map(|row| TableInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            comment: Some(format!("Temporal table — history: {}", str_val(row, 2))),
            row_estimate: None,
            size_bytes: None,
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            triggers: vec![],
            constraints: vec![],
        })
        .collect())
}

// ── Scheduled jobs ────────────────────────────────────────────────────────────

pub async fn fetch_agent_jobs(
    client: &mut SqlClient,
    caps: &VersionCapabilities,
) -> Result<Vec<ScheduledJobInfo>, String> {
    let q = SqlServerQueries::new(caps);
    let rows = run_query(client, q.list_agent_jobs()).await?;

    Ok(rows
        .iter()
        .map(|row| ScheduledJobInfo {
            name: str_val(row, 0),
            schema: None,
            enabled: bool_val(row, 1),
            definition: str_val(row, 2),
            schedule: str_val(row, 3),
            last_status: opt_str(row, 4),
            last_run_at: None,
            next_run_at: None,
            job_type: "sql_server_agent".into(),
        })
        .collect())
}

pub fn group_by_schema(
    tables: Vec<TableInfo>,
    views: Vec<ViewInfo>,
    functions: Vec<FunctionInfo>,
    procedures: Vec<ProcedureInfo>,
    sequences: Vec<SequenceInfo>,
    types: Vec<TypeInfo>,
) -> Vec<SchemaCatalogue> {
    let mut schema_names: Vec<String> = tables
        .iter()
        .map(|t| t.schema.clone())
        .chain(views.iter().map(|v| v.schema.clone()))
        .chain(functions.iter().map(|f| f.schema.clone()))
        .chain(procedures.iter().map(|p| p.schema.clone()))
        .collect();

    schema_names.sort();
    schema_names.dedup();

    schema_names
        .into_iter()
        .map(|name| SchemaCatalogue {
            tables: tables
                .iter()
                .filter(|t| t.schema == name)
                .cloned()
                .collect(),
            views: views.iter().filter(|v| v.schema == name).cloned().collect(),
            functions: functions
                .iter()
                .filter(|f| f.schema == name)
                .cloned()
                .collect(),
            procedures: procedures
                .iter()
                .filter(|p| p.schema == name)
                .cloned()
                .collect(),
            sequences: sequences
                .iter()
                .filter(|s| s.schema == name)
                .cloned()
                .collect(),
            types: types.iter().filter(|t| t.schema == name).cloned().collect(),
            triggers: vec![],           // attached to tables
            materialized_views: vec![], // SQL Server uses indexed views — not mat views
            extensions: vec![],         // not applicable
            packages: vec![],           // not applicable
            name,
        })
        .collect()
}
