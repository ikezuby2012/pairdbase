use crate::features::connections::drivers::versions::VersionCapabilities;

pub struct SqlServerQueries<'a> {
    caps: &'a VersionCapabilities,
}

impl<'a> SqlServerQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    // ── Tables ────────────────────────────────────────────────────────────────

    pub fn list_tables(&self) -> &'static str {
        r#"
        SELECT
            s.name                          AS schema_name,
            t.name                          AS table_name,
            ep.value                        AS table_comment,
            SUM(p.rows)                     AS row_estimate,
            SUM(a.total_pages) * 8 * 1024   AS size_bytes
        FROM sys.tables t
        JOIN sys.schemas s ON s.schema_id = t.schema_id
        JOIN sys.indexes i
            ON i.object_id = t.object_id AND i.index_id <= 1
        JOIN sys.partitions p
            ON p.object_id = t.object_id AND p.index_id = i.index_id
        JOIN sys.allocation_units a
            ON a.container_id = p.partition_id
        LEFT JOIN sys.extended_properties ep
            ON  ep.major_id  = t.object_id
            AND ep.minor_id  = 0
            AND ep.name      = 'MS_Description'
        WHERE t.is_ms_shipped = 0
        GROUP BY s.name, t.name, ep.value
        ORDER BY s.name, t.name
        "#
    }

    pub fn list_columns(&self) -> &'static str {
        r#"
        SELECT
            s.name              AS schema_name,
            t.name              AS table_name,
            c.name              AS col_name,
            tp.name             AS data_type,
            c.is_nullable,
            dc.definition       AS col_default,
            c.max_length,
            c.precision         AS numeric_precision,
            CAST(ep.value AS NVARCHAR(MAX)) AS col_comment
        FROM sys.columns c
        JOIN sys.tables t       ON t.object_id = c.object_id
        JOIN sys.schemas s      ON s.schema_id = t.schema_id
        JOIN sys.types tp       ON tp.user_type_id = c.user_type_id
        LEFT JOIN sys.default_constraints dc
            ON dc.object_id = c.default_object_id
        LEFT JOIN sys.extended_properties ep
            ON  ep.major_id  = c.object_id
            AND ep.minor_id  = c.column_id
            AND ep.name      = 'MS_Description'
        WHERE t.is_ms_shipped = 0
        ORDER BY s.name, t.name, c.column_id
        "#
    }

    // ── Indexes — version branched ────────────────────────────────────────────

    pub fn list_indexes(&self) -> String {
        let col_agg = if self.caps.string_agg {
            // SQL Server 2017+
            "STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal)"
        } else {
            // SQL Server 2012–2016
            "STUFF((
                SELECT ',' + c2.name
                FROM sys.index_columns ic2
                JOIN sys.columns c2
                    ON  c2.object_id = ic2.object_id
                    AND c2.column_id = ic2.column_id
                WHERE ic2.object_id = i.object_id
                  AND ic2.index_id  = i.index_id
                ORDER BY ic2.key_ordinal
                FOR XML PATH('')
            ), 1, 1, '')"
        };

        format!(
            r#"
            SELECT
                s.name              AS schema_name,
                t.name              AS table_name,
                i.name              AS index_name,
                i.is_unique,
                i.is_primary_key,
                i.type_desc         AS index_type,
                {col_agg}           AS columns,
                i.filter_definition AS condition
            FROM sys.indexes i
            JOIN sys.tables t
                ON t.object_id = i.object_id
            JOIN sys.schemas s
                ON s.schema_id = t.schema_id
            JOIN sys.index_columns ic
                ON  ic.object_id = i.object_id
                AND ic.index_id  = i.index_id
            JOIN sys.columns c
                ON  c.object_id = ic.object_id
                AND c.column_id = ic.column_id
            WHERE t.is_ms_shipped = 0
              AND i.name IS NOT NULL
            GROUP BY s.name, t.name, i.name, i.is_unique,
                     i.is_primary_key, i.type_desc, i.filter_definition
            ORDER BY s.name, t.name, i.name
            "#
        )
    }

    // ── Foreign keys ──────────────────────────────────────────────────────────

    pub fn list_foreign_keys(&self) -> &'static str {
        r#"
        SELECT
            s.name              AS schema_name,
            tp.name             AS table_name,
            fk.name             AS fk_name,
            cp.name             AS col_name,
            rs.name             AS ref_schema,
            rt.name             AS ref_table,
            cr.name             AS ref_col,
            fk.delete_referential_action_desc AS on_delete,
            fk.update_referential_action_desc AS on_update
        FROM sys.foreign_keys fk
        JOIN sys.tables tp
            ON tp.object_id = fk.parent_object_id
        JOIN sys.schemas s
            ON s.schema_id = tp.schema_id
        JOIN sys.tables rt
            ON rt.object_id = fk.referenced_object_id
        JOIN sys.schemas rs
            ON rs.schema_id = rt.schema_id
        JOIN sys.foreign_key_columns fkc
            ON fkc.constraint_object_id = fk.object_id
        JOIN sys.columns cp
            ON  cp.object_id = fkc.parent_object_id
            AND cp.column_id = fkc.parent_column_id
        JOIN sys.columns cr
            ON  cr.object_id = fkc.referenced_object_id
            AND cr.column_id = fkc.referenced_column_id
        ORDER BY s.name, tp.name
        "#
    }

    pub fn list_constraints(&self) -> &'static str {
        r#"
        SELECT
            s.name          AS schema_name,
            t.name          AS table_name,
            cc.name         AS con_name,
            'CHECK'         AS con_type,
            cc.definition
        FROM sys.check_constraints cc
        JOIN sys.tables t   ON t.object_id = cc.parent_object_id
        JOIN sys.schemas s  ON s.schema_id  = t.schema_id
        WHERE t.is_ms_shipped = 0
        ORDER BY s.name, t.name
        "#
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn list_views(&self) -> &'static str {
        r#"
        SELECT
            s.name                          AS schema_name,
            v.name                          AS view_name,
            OBJECT_DEFINITION(v.object_id)  AS definition,
            v.with_check_option             AS is_updatable
        FROM sys.views v
        JOIN sys.schemas s ON s.schema_id = v.schema_id
        WHERE v.is_ms_shipped = 0
        ORDER BY s.name, v.name
        "#
    }

    // ── Triggers ──────────────────────────────────────────────────────────────

    pub fn list_triggers(&self) -> &'static str {
        r#"
        SELECT
            s.name          AS schema_name,
            t.name          AS table_name,
            tr.name         AS trigger_name,
            tr.is_disabled,
            CASE
                WHEN OBJECTPROPERTY(tr.object_id, 'ExecIsAfterTrigger')     = 1 THEN 'AFTER'
                WHEN OBJECTPROPERTY(tr.object_id, 'ExecIsInsteadOfTrigger') = 1 THEN 'INSTEAD OF'
                ELSE 'AFTER'
            END             AS timing,
            CASE WHEN OBJECTPROPERTY(tr.object_id, 'ExecIsInsertTrigger') = 1 THEN 'INSERT,' ELSE '' END +
            CASE WHEN OBJECTPROPERTY(tr.object_id, 'ExecIsUpdateTrigger') = 1 THEN 'UPDATE,' ELSE '' END +
            CASE WHEN OBJECTPROPERTY(tr.object_id, 'ExecIsDeleteTrigger') = 1 THEN 'DELETE'  ELSE '' END
                            AS events,
            OBJECT_DEFINITION(tr.object_id) AS definition
        FROM sys.triggers tr
        JOIN sys.tables t   ON t.object_id = tr.parent_id
        JOIN sys.schemas s  ON s.schema_id  = t.schema_id
        WHERE tr.is_ms_shipped = 0
        ORDER BY s.name, t.name, tr.name
        "#
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    pub fn list_functions(&self) -> &'static str {
        r#"
        SELECT
            s.name                              AS schema_name,
            o.name                              AS fn_name,
            o.type_desc                         AS fn_type,
            OBJECT_DEFINITION(o.object_id)      AS definition
        FROM sys.objects o
        JOIN sys.schemas s ON s.schema_id = o.schema_id
        WHERE o.type IN ('FN', 'IF', 'TF')
          AND o.is_ms_shipped = 0
        ORDER BY s.name, o.name
        "#
    }

    // ── Procedures ────────────────────────────────────────────────────────────

    pub fn list_procedures(&self) -> &'static str {
        r#"
        SELECT
            s.name                          AS schema_name,
            p.name                          AS proc_name,
            OBJECT_DEFINITION(p.object_id)  AS definition
        FROM sys.procedures p
        JOIN sys.schemas s ON s.schema_id = p.schema_id
        WHERE p.is_ms_shipped = 0
        ORDER BY s.name, p.name
        "#
    }

    pub fn list_procedure_params(&self) -> &'static str {
        r#"
        SELECT
            s.name          AS schema_name,
            o.name          AS proc_name,
            p.name          AS param_name,
            tp.name         AS data_type,
            p.is_output,
            p.has_default_value,
            CAST(p.default_value AS NVARCHAR(MAX)) AS default_val
        FROM sys.parameters p
        JOIN sys.objects o  ON o.object_id      = p.object_id
        JOIN sys.schemas s  ON s.schema_id       = o.schema_id
        JOIN sys.types   tp ON tp.user_type_id   = p.user_type_id
        WHERE o.type = 'P'
          AND o.is_ms_shipped = 0
        ORDER BY s.name, o.name, p.parameter_id
        "#
    }

    // ── Sequences — version gated ─────────────────────────────────────────────

    pub fn list_sequences(&self) -> Option<&'static str> {
        if !self.caps.sequences {
            return None;  // SQL Server < 2012 — caller skips
        }
        Some(r#"
        SELECT
            s.name              AS schema_name,
            seq.name            AS seq_name,
            tp.name             AS data_type,
            seq.start_value,
            seq.minimum_value,
            seq.maximum_value,
            seq.increment,
            seq.is_cycling,
            seq.current_value
        FROM sys.sequences seq
        JOIN sys.schemas s  ON s.schema_id     = seq.schema_id
        JOIN sys.types   tp ON tp.user_type_id = seq.user_type_id
        ORDER BY s.name, seq.name
        "#)
    }

    // ── Temporal tables — version gated ──────────────────────────────────────

    pub fn list_temporal_tables(&self) -> Option<&'static str> {
        if !self.caps.temporal_tables {
            return None;  // SQL Server < 2016
        }
        Some(r#"
        SELECT
            s.name      AS schema_name,
            t.name      AS table_name,
            ht.name     AS history_table_name
        FROM sys.tables t
        JOIN sys.schemas s  ON s.schema_id   = t.schema_id
        LEFT JOIN sys.tables ht ON ht.object_id = t.history_table_id
        WHERE t.temporal_type = 2
        ORDER BY s.name, t.name
        "#)
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    pub fn list_types(&self) -> &'static str {
        r#"
        SELECT
            s.name          AS schema_name,
            t.name          AS type_name,
            bt.name         AS base_type,
            t.is_table_type
        FROM sys.types t
        JOIN sys.schemas s  ON s.schema_id      = t.schema_id
        JOIN sys.types   bt ON bt.user_type_id  = t.system_type_id
        WHERE t.is_user_defined = 1
          AND s.name NOT IN ('sys')
        ORDER BY s.name, t.name
        "#
    }

    // ── Scheduled jobs ────────────────────────────────────────────────────────

    pub fn list_agent_jobs(&self) -> &'static str {
        r#"
        SELECT
            j.name      AS job_name,
            j.enabled,
            js.command  AS definition,
            CASE sc.freq_type
                WHEN 1   THEN 'Once'
                WHEN 4   THEN 'Daily every ' + CAST(sc.freq_interval AS VARCHAR) + ' day(s)'
                WHEN 8   THEN 'Weekly'
                WHEN 16  THEN 'Monthly on day ' + CAST(sc.freq_interval AS VARCHAR)
                WHEN 64  THEN 'On SQL Server Agent start'
                WHEN 128 THEN 'On idle'
                ELSE 'Unknown'
            END         AS schedule,
            CASE ja.last_run_outcome
                WHEN 1 THEN 'succeeded'
                WHEN 0 THEN 'failed'
                WHEN 3 THEN 'cancelled'
                ELSE NULL
            END         AS last_status
        FROM msdb.dbo.sysjobs j
        LEFT JOIN msdb.dbo.sysjobsteps js
            ON js.job_id = j.job_id AND js.step_id = 1
        LEFT JOIN msdb.dbo.sysjobschedules jsc
            ON jsc.job_id = j.job_id
        LEFT JOIN msdb.dbo.sysschedules sc
            ON sc.schedule_id = jsc.schedule_id
        LEFT JOIN msdb.dbo.sysjobactivity ja
            ON ja.job_id = j.job_id
           AND ja.session_id = (
               SELECT MAX(session_id)
               FROM msdb.dbo.sysjobactivity
               WHERE job_id = j.job_id
           )
        ORDER BY j.name
        "#
    }
}