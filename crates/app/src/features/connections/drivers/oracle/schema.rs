use oracle::Connection as OracleConn;

use super::queries::OracleQueries;
use crate::features::connections::drivers::{
    trait_::{
        ArgumentInfo, ColumnInfo, ConstraintInfo, ForeignKey, FunctionInfo, IndexInfo,
        MaterializedViewInfo, PackageInfo, ProcedureInfo, ScheduledJobInfo, SchemaCatalogue,
        SchemaInfo, SequenceInfo, TableInfo, TriggerInfo, TypeInfo, ViewInfo,
    },
    versions::VersionCapabilities,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn str_val(row: &oracle::Row, idx: usize) -> String {
    row.get::<_, String>(idx).unwrap_or_default()
}

fn opt_str(row: &oracle::Row, idx: usize) -> Option<String> {
    row.get::<_, String>(idx).ok().filter(|s| !s.is_empty())
}

fn i64_val(row: &oracle::Row, idx: usize) -> i64 {
    row.get::<_, i64>(idx).unwrap_or(0)
}

fn bool_from_str(row: &oracle::Row, idx: usize, true_val: &str) -> bool {
    row.get::<_, String>(idx)
        .map(|s| s == true_val)
        .unwrap_or(false)
}

fn collect_rows(conn: &OracleConn, sql: &str) -> Result<Vec<oracle::Row>, String> {
    let mut cursor = conn.query(sql, &[]).map_err(|e| e.to_string())?;
    let mut rows = vec![];
    for result in cursor.by_ref() {
        rows.push(result.map_err(|e| e.to_string())?);
    }
    Ok(rows)
}

// ── Tables ────────────────────────────────────────────────────────────────────

pub fn fetch_tables(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<TableInfo>, String> {
    let q = OracleQueries::new(caps);

    let table_rows = collect_rows(conn, q.list_tables())?;

    let mut tables: Vec<TableInfo> = table_rows
        .iter()
        .map(|row| TableInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            comment: opt_str(row, 2),
            row_estimate: row.get::<_, i64>(3).ok(),
            size_bytes: row.get::<_, i64>(4).ok(),
            columns: vec![],
            indexes: vec![],
            foreign_keys: vec![],
            triggers: vec![],
            constraints: vec![],
        })
        .collect();

    fetch_columns(conn, caps, &mut tables)?;
    fetch_indexes(conn, caps, &mut tables)?;
    fetch_foreign_keys(conn, caps, &mut tables)?;
    fetch_constraints(conn, caps, &mut tables)?;
    fetch_triggers(conn, caps, &mut tables)?;

    Ok(tables)
}

fn fetch_columns(
    conn: &OracleConn,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_columns())?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            t.columns.push(ColumnInfo {
                name: str_val(row, 2),
                data_type: normalize_type(&str_val(row, 3)),
                nullable: bool_from_str(row, 4, "Y"),
                default: opt_str(row, 5),
                max_length: row.get::<_, i64>(6).ok(),
                numeric_precision: row.get::<_, i64>(7).ok(),
                comment: opt_str(row, 8),
                is_primary: false, // resolved in fetch_indexes
                is_indexed: false,
                is_unique: false,
            });
        }
    }

    Ok(())
}

fn fetch_indexes(
    conn: &OracleConn,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_indexes())?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let is_unique = str_val(row, 3) == "UNIQUE";
            let is_primary = i64_val(row, 4) == 1;

            let col_names: Vec<String> = str_val(row, 6)
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Resolve column flags from index membership
            for col in &mut t.columns {
                if col_names.contains(&col.name) {
                    col.is_indexed = true;
                    if is_primary {
                        col.is_primary = true;
                    }
                    if is_unique {
                        col.is_unique = true;
                    }
                }
            }

            t.indexes.push(IndexInfo {
                name: str_val(row, 2),
                unique: is_unique,
                primary: is_primary,
                index_type: str_val(row, 5),
                columns: col_names,
                condition: None, // Oracle partial indexes via function-based
            });
        }
    }

    Ok(())
}

fn fetch_foreign_keys(
    conn: &OracleConn,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_foreign_keys())?;

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
                on_update: "NO ACTION".into(), // Oracle has no ON UPDATE CASCADE
            });
        }
    }

    Ok(())
}

fn fetch_constraints(
    conn: &OracleConn,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_constraints())?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let raw_type = str_val(row, 3);
            let con_type = match raw_type.as_str() {
                "C" => "CHECK",
                "U" => "UNIQUE",
                _ => "UNKNOWN",
            };

            t.constraints.push(ConstraintInfo {
                name: str_val(row, 2),
                constraint_type: con_type.into(),
                definition: str_val(row, 4),
            });
        }
    }

    Ok(())
}

fn fetch_triggers(
    conn: &OracleConn,
    caps: &VersionCapabilities,
    tables: &mut Vec<TableInfo>,
) -> Result<(), String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_triggers())?;

    for row in &rows {
        let schema = str_val(row, 0);
        let tname = str_val(row, 1);

        if let Some(t) = tables
            .iter_mut()
            .find(|t| t.schema == schema && t.name == tname)
        {
            let trig_type = str_val(row, 3);
            let timing = if trig_type.contains("BEFORE") {
                "BEFORE"
            } else if trig_type.contains("INSTEAD OF") {
                "INSTEAD OF"
            } else {
                "AFTER"
            };

            let events: Vec<String> = str_val(row, 4)
                .split(" OR ")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            t.triggers.push(TriggerInfo {
                schema: schema.clone(),
                name: str_val(row, 2),
                table_name: tname,
                timing: timing.into(),
                events,
                definition: str_val(row, 5),
                enabled: bool_from_str(row, 6, "ENABLED"),
                language: "PL/SQL".into(),
            });
        }
    }

    Ok(())
}

// ── Views ─────────────────────────────────────────────────────────────────────

pub fn fetch_views(conn: &OracleConn, caps: &VersionCapabilities) -> Result<Vec<ViewInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_views())?;

    Ok(rows
        .iter()
        .map(|row| ViewInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            definition: str_val(row, 2),
            is_updatable: false, // Oracle requires WITH CHECK OPTION — checked separately
            columns: vec![],
        })
        .collect())
}

// ── Materialized views ────────────────────────────────────────────────────────

pub fn fetch_materialized_views(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<MaterializedViewInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_materialized_views())?;

    Ok(rows
        .iter()
        .map(|row| MaterializedViewInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            definition: str_val(row, 2),
            auto_refresh: str_val(row, 3) == "COMMIT",
            last_refresh_at: None, // LAST_REFRESH_DATE needs chrono parsing — optional
            columns: vec![],
        })
        .collect())
}

// ── Functions ─────────────────────────────────────────────────────────────────

pub fn fetch_functions(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<FunctionInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_functions())?;

    // Aggregate source lines per function (OWNER, NAME, TEXT per line)
    let mut map: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();

    for row in &rows {
        let owner = str_val(row, 0);
        let name = str_val(row, 1);
        let text = str_val(row, 2);
        map.entry((owner, name)).or_default().push_str(&text);
    }

    Ok(map
        .into_iter()
        .map(|((owner, name), definition)| {
            // Parse return type from first line: FUNCTION name RETURN <type>
            let return_type = definition
                .lines()
                .next()
                .and_then(|l| {
                    let upper = l.to_uppercase();
                    upper.find("RETURN").map(|pos| {
                        l[pos + 6..]
                            .trim()
                            .split_whitespace()
                            .next()
                            .unwrap_or("UNKNOWN")
                            .to_string()
                    })
                })
                .unwrap_or_else(|| "UNKNOWN".into());

            FunctionInfo {
                schema: owner,
                name,
                language: "PL/SQL".into(),
                return_type,
                definition,
                is_aggregate: false,
                is_window: false,
                volatility: None,
                arguments: vec![],
            }
        })
        .collect())
}

// ── Procedures ────────────────────────────────────────────────────────────────

pub fn fetch_procedures(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<ProcedureInfo>, String> {
    let q = OracleQueries::new(caps);

    // Aggregate source lines
    let source_rows = collect_rows(conn, q.list_procedures())?;
    let mut map: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();

    for row in &source_rows {
        let owner = str_val(row, 0);
        let name = str_val(row, 1);
        let text = str_val(row, 2);
        map.entry((owner, name)).or_default().push_str(&text);
    }

    let mut procs: Vec<ProcedureInfo> = map
        .into_iter()
        .map(|((owner, name), definition)| ProcedureInfo {
            schema: owner,
            name,
            language: "PL/SQL".into(),
            definition,
            arguments: vec![],
        })
        .collect();

    // Fetch and attach arguments
    let arg_rows = collect_rows(conn, q.list_procedure_args())?;

    for row in &arg_rows {
        let owner = str_val(row, 0);
        let pname = str_val(row, 1);

        if let Some(proc) = procs
            .iter_mut()
            .find(|p| p.schema == owner && p.name == pname)
        {
            proc.arguments.push(ArgumentInfo {
                name: opt_str(row, 2),
                data_type: str_val(row, 3),
                mode: str_val(row, 4),
                default: opt_str(row, 5),
            });
        }
    }

    Ok(procs)
}

// ── Sequences ─────────────────────────────────────────────────────────────────

pub fn fetch_sequences(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<SequenceInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_sequences())?;

    Ok(rows
        .iter()
        .map(|row| SequenceInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            data_type: "NUMBER".into(),
            min_value: i64_val(row, 2),
            max_value: i64_val(row, 3),
            start_value: i64_val(row, 2), // Oracle doesn't store original start separately
            increment: i64_val(row, 4),
            cycle: bool_from_str(row, 5, "Y"),
            last_value: row.get::<_, i64>(6).ok(),
        })
        .collect())
}

// ── Types ─────────────────────────────────────────────────────────────────────

pub fn fetch_types(conn: &OracleConn, caps: &VersionCapabilities) -> Result<Vec<TypeInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_types())?;

    Ok(rows
        .iter()
        .map(|row| TypeInfo {
            schema: str_val(row, 0),
            name: str_val(row, 1),
            type_: str_val(row, 2).to_lowercase(),
            values: vec![], // populated separately for OBJECT types if needed
            definition: None,
        })
        .collect())
}

// ── Packages ──────────────────────────────────────────────────────────────────

pub fn fetch_packages(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<PackageInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_packages())?;

    // Aggregate: key = (owner, name), value = (spec_text, body_text)
    let mut map: std::collections::HashMap<(String, String), (String, String)> =
        std::collections::HashMap::new();

    for row in &rows {
        let owner = str_val(row, 0);
        let name = str_val(row, 1);
        let type_ = str_val(row, 2);
        let text = str_val(row, 3);

        let entry = map.entry((owner, name)).or_insert(("".into(), "".into()));

        if type_ == "PACKAGE" {
            entry.0.push_str(&text); // spec
        } else {
            entry.1.push_str(&text); // body
        }
    }

    Ok(map
        .into_iter()
        .map(|((owner, name), (header, body))| PackageInfo {
            schema: owner,
            name,
            header,
            body,
            status: "VALID".into(),
        })
        .collect())
}

// ── Scheduled jobs ────────────────────────────────────────────────────────────

pub fn fetch_jobs(
    conn: &OracleConn,
    caps: &VersionCapabilities,
) -> Result<Vec<ScheduledJobInfo>, String> {
    let q = OracleQueries::new(caps);
    let rows = collect_rows(conn, q.list_jobs())?;

    Ok(rows
        .iter()
        .map(|row| ScheduledJobInfo {
            name: str_val(row, 0),
            schema: Some(str_val(row, 1)),
            enabled: bool_from_str(row, 2, "TRUE"),
            schedule: opt_str(row, 3).unwrap_or_else(|| "unknown".into()),
            last_status: opt_str(row, 4),
            definition: str_val(row, 5),
            last_run_at: None,
            next_run_at: None,
            job_type: "dbms_scheduler".into(),
        })
        .collect())
}

// ── Group into SchemaCatalogues ───────────────────────────────────────────────

pub fn group_by_schema(
    tables: Vec<TableInfo>,
    views: Vec<ViewInfo>,
    mat_views: Vec<MaterializedViewInfo>,
    functions: Vec<FunctionInfo>,
    procedures: Vec<ProcedureInfo>,
    sequences: Vec<SequenceInfo>,
    types: Vec<TypeInfo>,
    packages: Vec<PackageInfo>,
) -> Vec<SchemaCatalogue> {
    let mut schema_names: Vec<String> = tables
        .iter()
        .map(|t| t.schema.clone())
        .chain(views.iter().map(|v| v.schema.clone()))
        .chain(mat_views.iter().map(|m| m.schema.clone()))
        .chain(functions.iter().map(|f| f.schema.clone()))
        .chain(procedures.iter().map(|p| p.schema.clone()))
        .chain(packages.iter().map(|p| p.schema.clone()))
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
            materialized_views: mat_views
                .iter()
                .filter(|m| m.schema == name)
                .cloned()
                .collect(),
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
            packages: packages
                .iter()
                .filter(|p| p.schema == name)
                .cloned()
                .collect(),
            triggers: vec![],   // attached to tables
            extensions: vec![], // not applicable to Oracle
            name,
        })
        .collect()
}

// ── Type normaliser ───────────────────────────────────────────────────────────

fn normalize_type(t: &str) -> String {
    match t {
        "VARCHAR2" => "varchar2",
        "NVARCHAR2" => "nvarchar2",
        "NUMBER" => "number",
        "FLOAT" => "float",
        "DATE" => "date",
        "CHAR" => "char",
        "NCHAR" => "nchar",
        "CLOB" => "clob",
        "BLOB" => "blob",
        "XMLTYPE" => "xmltype",
        "RAW" => "raw",
        "LONG" => "long",
        other => return other.to_lowercase(),
    }
    .to_string()
}
