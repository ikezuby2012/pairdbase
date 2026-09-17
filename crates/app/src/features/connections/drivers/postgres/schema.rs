use futures::future::BoxFuture;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row, AssertSqlSafe};

use crate::features::connections::drivers::{
    trait_::{
        ArgumentInfo, ColumnInfo, ConstraintInfo, ExtensionInfo, ForeignKey,
        FunctionInfo, IndexInfo, MaterializedViewInfo, PackageInfo,
        ProcedureInfo, ScheduledJobInfo, SchemaCatalogue, SchemaInfo,
        SequenceInfo, TableInfo, TriggerInfo, TypeInfo, ViewInfo,
    },
    versions::VersionCapabilities,
};
use super::queries::PostgresQueries;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn str_val(row: &PgRow, col: &str) -> String {
    row.try_get::<String, _>(col).unwrap_or_default()
}

fn opt_str(row: &PgRow, col: &str) -> Option<String> {
    row.try_get::<String, _>(col)
        .ok()
        .filter(|s| !s.is_empty())
}

fn bool_val(row: &PgRow, col: &str) -> bool {
    row.try_get::<bool, _>(col).unwrap_or(false)
}

fn i64_val(row: &PgRow, col: &str) -> i64 {
    row.try_get::<i64, _>(col).unwrap_or(0)
}

fn run_query(
    pool: &PgPool,
    sql: &str,
) -> BoxFuture<'static, Result<Vec<PgRow>, String>> {
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
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = PostgresQueries::new(caps);

    let table_rows = run_query(pool, q.list_tables()).await?;

    let mut tables: Vec<TableInfo> = table_rows.iter().map(|row| TableInfo {
        schema:       str_val(row, "schema_name"),
        name:         str_val(row, "table_name"),
        comment:      opt_str(row, "table_comment"),
        row_estimate: row.try_get::<i64, _>("row_estimate").ok(),
        size_bytes:   row.try_get::<i64, _>("size_bytes").ok(),
        columns:      vec![],
        indexes:      vec![],
        foreign_keys: vec![],
        triggers:     vec![],
        constraints:  vec![],
    }).collect();

    fetch_columns(pool, caps, &mut tables).await?;
    fetch_indexes(pool, caps, &mut tables).await?;
    fetch_foreign_keys(pool, caps, &mut tables).await?;
    fetch_constraints(pool, caps, &mut tables).await?;
    fetch_triggers(pool, caps, &mut tables).await?;

    Ok(tables)
}

async fn fetch_columns(
    pool:   &PgPool,
    caps:   &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_columns()).await?;

    for row in &rows {
        let schema = str_val(row, "schema_name");
        let tname  = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.columns.push(ColumnInfo {
                name:              str_val(row, "col_name"),
                data_type:         str_val(row, "data_type"),
                nullable:          bool_val(row, "nullable"),
                default:           opt_str(row, "col_default"),
                max_length:        row.try_get::<i64, _>("max_length").ok(),
                numeric_precision: None,
                comment:           opt_str(row, "col_comment"),
                is_primary:        bool_val(row, "is_primary"),
                is_indexed:        bool_val(row, "is_indexed"),
                is_unique:         bool_val(row, "is_unique"),
            });
        }
    }

    Ok(())
}

async fn fetch_indexes(
    pool:   &PgPool,
    caps:   &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_indexes()).await?;

    for row in &rows {
        let schema = str_val(row, "schema_name");
        let tname  = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let col_names: Vec<String> = row
                .try_get::<Vec<String>, _>("columns")
                .unwrap_or_default();

            t.indexes.push(IndexInfo {
                name:       str_val(row, "index_name"),
                unique:     bool_val(row, "is_unique"),
                primary:    bool_val(row, "is_primary"),
                index_type: str_val(row, "index_type"),
                columns:    col_names,
                condition:  opt_str(row, "condition"),
            });
        }
    }

    Ok(())
}

async fn fetch_foreign_keys(
    pool:   &PgPool,
    caps:   &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_foreign_keys()).await?;

    for row in &rows {
        let schema = str_val(row, "schema_name");
        let tname  = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.foreign_keys.push(ForeignKey {
                name:       str_val(row, "fk_name"),
                column:     str_val(row, "col_name"),
                ref_schema: str_val(row, "ref_schema"),
                ref_table:  str_val(row, "ref_table"),
                ref_column: str_val(row, "ref_col"),
                on_delete:  map_pg_action(str_val(row, "on_delete").as_str()),
                on_update:  map_pg_action(str_val(row, "on_update").as_str()),
            });
        }
    }

    Ok(())
}

async fn fetch_constraints(
    pool:   &PgPool,
    caps:   &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_constraints()).await?;

    for row in &rows {
        let schema = str_val(row, "schema_name");
        let tname  = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let raw_type = str_val(row, "con_type");
            let con_type = match raw_type.as_str() {
                "c" => "CHECK",
                "u" => "UNIQUE",
                _   => "UNKNOWN",
            };

            t.constraints.push(ConstraintInfo {
                name:            str_val(row, "con_name"),
                constraint_type: con_type.into(),
                definition:      str_val(row, "definition"),
            });
        }
    }

    Ok(())
}

async fn fetch_triggers(
    pool:   &PgPool,
    caps:   &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_triggers()).await?;

    for row in &rows {
        let schema = str_val(row, "schema_name");
        let tname  = str_val(row, "table_name");

        if let Some(t) = tables.iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let def = str_val(row, "definition");
            let (timing, events) = parse_trigger_def(&def);

            t.triggers.push(TriggerInfo {
                schema:     schema.clone(),
                name:       str_val(row, "trigger_name"),
                table_name: tname,
                timing,
                events,
                definition: def,
                enabled:    str_val(row, "enabled") != "D",
                language:   "plpgsql".into(),
            });
        }
    }

    Ok(())
}

// ── Views ─────────────────────────────────────────────────────────────────────

pub async fn fetch_views(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ViewInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_views()).await?;

    Ok(rows.iter().map(|row| ViewInfo {
        schema:       str_val(row, "schema_name"),
        name:         str_val(row, "view_name"),
        definition:   str_val(row, "definition"),
        is_updatable: false,
        columns:      vec![],
    }).collect())
}

// ── Materialized views ────────────────────────────────────────────────────────

pub async fn fetch_materialized_views(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<MaterializedViewInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_materialized_views()).await?;

    Ok(rows.iter().map(|row| MaterializedViewInfo {
        schema:          str_val(row, "schema_name"),
        name:            str_val(row, "view_name"),
        definition:      str_val(row, "definition"),
        last_refresh_at: None,
        auto_refresh:    false,
        columns:         vec![],
    }).collect())
}

// ── Functions ─────────────────────────────────────────────────────────────────

pub async fn fetch_functions(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<FunctionInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_functions()).await?;

    Ok(rows.iter().map(|row| FunctionInfo {
        schema:       str_val(row, "schema_name"),
        name:         str_val(row, "fn_name"),
        language:     str_val(row, "language"),
        return_type:  str_val(row, "return_type"),
        definition:   str_val(row, "definition"),
        is_aggregate: bool_val(row, "is_aggregate"),
        is_window:    false,
        volatility:   opt_str(row, "volatility"),
        arguments:    vec![],
    }).collect())
}

// ── Procedures ────────────────────────────────────────────────────────────────

pub async fn fetch_procedures(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ProcedureInfo>, String> {
    let q = PostgresQueries::new(caps);

    let sql = match q.list_procedures() {
        Some(s) => s,
        None    => {
            tracing::debug!("procedures not supported on this PostgreSQL version — skipping");
            return Ok(vec![]);
        }
    };

    let rows = run_query(pool, sql).await?;

    Ok(rows.iter().map(|row| ProcedureInfo {
        schema:     str_val(row, "schema_name"),
        name:       str_val(row, "proc_name"),
        language:   str_val(row, "language"),
        definition: str_val(row, "definition"),
        arguments:  vec![],
    }).collect())
}

// ── Sequences ─────────────────────────────────────────────────────────────────

pub async fn fetch_sequences(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<SequenceInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_sequences()).await?;

    Ok(rows.iter().map(|row| SequenceInfo {
        schema:      str_val(row, "schema_name"),
        name:        str_val(row, "seq_name"),
        data_type:   str_val(row, "data_type"),
        start_value: i64_val(row, "start_value"),
        min_value:   i64_val(row, "min_value"),
        max_value:   i64_val(row, "max_value"),
        increment:   i64_val(row, "increment"),
        cycle:       bool_val(row, "cycle"),
        last_value:  None,
    }).collect())
}

// ── Types ─────────────────────────────────────────────────────────────────────

pub async fn fetch_types(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<TypeInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_types()).await?;

    Ok(rows.iter().map(|row| TypeInfo {
        schema:     str_val(row, "schema_name"),
        name:       str_val(row, "type_name"),
        type_:      str_val(row, "type_kind"),
        values:     row.try_get::<Vec<String>, _>("enum_values")
                       .unwrap_or_default(),
        definition: None,
    }).collect())
}

// ── Extensions ────────────────────────────────────────────────────────────────

pub async fn fetch_extensions(
    pool: &PgPool,
    caps: &VersionCapabilities,
) -> Result<Vec<ExtensionInfo>, String> {
    let q    = PostgresQueries::new(caps);
    let rows = run_query(pool, q.list_extensions()).await?;

    Ok(rows.iter().map(|row| ExtensionInfo {
        name:    str_val(row, "name"),
        version: str_val(row, "version"),
        schema:  str_val(row, "schema_name"),
    }).collect())
}

// ── Group into SchemaCatalogues ───────────────────────────────────────────────

pub fn group_by_schema(
    tables:     Vec<TableInfo>,
    views:      Vec<ViewInfo>,
    mat_views:  Vec<MaterializedViewInfo>,
    functions:  Vec<FunctionInfo>,
    procedures: Vec<ProcedureInfo>,
    sequences:  Vec<SequenceInfo>,
    types:      Vec<TypeInfo>,
    extensions: Vec<ExtensionInfo>,
) -> Vec<SchemaCatalogue> {
    let mut schema_names: Vec<String> = tables.iter().map(|t| t.schema.clone())
        .chain(views.iter().map(|v| v.schema.clone()))
        .chain(functions.iter().map(|f| f.schema.clone()))
        .collect();

    schema_names.sort();
    schema_names.dedup();

    schema_names.into_iter().map(|name| SchemaCatalogue {
        tables:             tables.iter()
            .filter(|t| t.schema == name).cloned().collect(),
        views:              views.iter()
            .filter(|v| v.schema == name).cloned().collect(),
        materialized_views: mat_views.iter()
            .filter(|m| m.schema == name).cloned().collect(),
        functions:          functions.iter()
            .filter(|f| f.schema == name).cloned().collect(),
        procedures:         procedures.iter()
            .filter(|p| p.schema == name).cloned().collect(),
        sequences:          sequences.iter()
            .filter(|s| s.schema == name).cloned().collect(),
        types:              types.iter()
            .filter(|t| t.schema == name).cloned().collect(),
        extensions:         extensions.iter()
            .filter(|e| e.schema == name).cloned().collect(),
        triggers:           vec![],  // attached to tables
        packages:           vec![],  // PostgreSQL has no packages
        name,
    }).collect()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn map_pg_action(code: &str) -> String {
    match code {
        "a" => "NO ACTION",
        "r" => "RESTRICT",
        "c" => "CASCADE",
        "n" => "SET NULL",
        "d" => "SET DEFAULT",
        _   => "NO ACTION",
    }.to_string()
}

fn parse_trigger_def(def: &str) -> (String, Vec<String>) {
    let upper = def.to_uppercase();

    let timing = if upper.contains("BEFORE") {
        "BEFORE"
    } else if upper.contains("INSTEAD OF") {
        "INSTEAD OF"
    } else {
        "AFTER"
    }.to_string();

    let mut events = vec![];
    if upper.contains("INSERT")   { events.push("INSERT".into()); }
    if upper.contains("UPDATE")   { events.push("UPDATE".into()); }
    if upper.contains("DELETE")   { events.push("DELETE".into()); }
    if upper.contains("TRUNCATE") { events.push("TRUNCATE".into()); }

    (timing, events)
}