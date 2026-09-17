use crate::features::connections::drivers::versions::VersionCapabilities;

pub struct PostgresQueries<'a> {
    caps: &'a VersionCapabilities,
}

impl<'a> PostgresQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    // ── Tables ────────────────────────────────────────────────────────────────

    pub fn list_tables(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                       AS schema_name,
            c.relname                       AS table_name,
            obj_description(c.oid)          AS table_comment,
            c.reltuples::BIGINT             AS row_estimate,
            pg_total_relation_size(c.oid)   AS size_bytes
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
        ORDER BY n.nspname, c.relname
        "#
    }

    pub fn list_columns(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                                               AS schema_name,
            c.relname                                               AS table_name,
            a.attname                                               AS col_name,
            pg_catalog.format_type(a.atttypid, a.atttypmod)        AS data_type,
            NOT a.attnotnull                                        AS nullable,
            pg_get_expr(d.adbin, d.adrelid)                        AS col_default,
            CASE WHEN a.atttypmod > 0 THEN a.atttypmod - 4 END     AS max_length,
            col_description(a.attrelid, a.attnum)                   AS col_comment,
            EXISTS (
                SELECT 1 FROM pg_constraint pk
                WHERE pk.conrelid = a.attrelid
                  AND pk.contype  = 'p'
                  AND a.attnum    = ANY(pk.conkey)
            ) AS is_primary,
            EXISTS (
                SELECT 1 FROM pg_index i
                WHERE i.indrelid = a.attrelid
                  AND a.attnum   = ANY(i.indkey)
            ) AS is_indexed,
            EXISTS (
                SELECT 1 FROM pg_constraint u
                WHERE u.conrelid = a.attrelid
                  AND u.contype  = 'u'
                  AND a.attnum   = ANY(u.conkey)
            ) AS is_unique
        FROM pg_attribute a
        JOIN pg_class     c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_attrdef d
            ON d.adrelid = a.attrelid AND d.adnum = a.attnum
        WHERE a.attnum > 0
          AND NOT a.attisdropped
          AND c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
        ORDER BY n.nspname, c.relname, a.attnum
        "#
    }

    // ── Indexes ───────────────────────────────────────────────────────────────
    // No version branching needed — array_agg available in all PG versions we support

    pub fn list_indexes(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                               AS schema_name,
            t.relname                               AS table_name,
            i.relname                               AS index_name,
            ix.indisunique                          AS is_unique,
            ix.indisprimary                         AS is_primary,
            am.amname                               AS index_type,
            pg_get_expr(ix.indpred, ix.indrelid)    AS condition,
            array_agg(a.attname ORDER BY pos.ord)   AS columns
        FROM pg_index ix
        JOIN pg_class     t  ON t.oid = ix.indrelid
        JOIN pg_class     i  ON i.oid = ix.indexrelid
        JOIN pg_namespace n  ON n.oid = t.relnamespace
        JOIN pg_am        am ON am.oid = i.relam
        JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS pos(attnum, ord) ON true
        JOIN pg_attribute a
            ON a.attrelid = t.oid AND a.attnum = pos.attnum
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
        GROUP BY n.nspname, t.relname, i.relname, ix.indisunique,
                 ix.indisprimary, am.amname, ix.indpred, ix.indrelid
        ORDER BY n.nspname, t.relname, i.relname
        "#
    }

    // ── Foreign keys ──────────────────────────────────────────────────────────

    pub fn list_foreign_keys(&self) -> &'static str {
        r#"
        SELECT
            n.nspname               AS schema_name,
            c.relname               AS table_name,
            ct.conname              AS fk_name,
            a.attname               AS col_name,
            rn.nspname              AS ref_schema,
            rc.relname              AS ref_table,
            ra.attname              AS ref_col,
            ct.confdeltype::TEXT    AS on_delete,
            ct.confupdtype::TEXT    AS on_update
        FROM pg_constraint ct
        JOIN pg_class     c  ON c.oid  = ct.conrelid
        JOIN pg_namespace n  ON n.oid  = c.relnamespace
        JOIN pg_class     rc ON rc.oid = ct.confrelid
        JOIN pg_namespace rn ON rn.oid = rc.relnamespace
        JOIN pg_attribute a
            ON a.attrelid = ct.conrelid AND a.attnum = ANY(ct.conkey)
        JOIN pg_attribute ra
            ON ra.attrelid = ct.confrelid AND ra.attnum = ANY(ct.confkey)
        WHERE ct.contype = 'f'
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname
        "#
    }

    pub fn list_constraints(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                       AS schema_name,
            c.relname                       AS table_name,
            ct.conname                      AS con_name,
            ct.contype::TEXT                AS con_type,
            pg_get_constraintdef(ct.oid)    AS definition
        FROM pg_constraint ct
        JOIN pg_class     c ON c.oid = ct.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE ct.contype IN ('c', 'u')
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname
        "#
    }

    // ── Triggers ──────────────────────────────────────────────────────────────

    pub fn list_triggers(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                       AS schema_name,
            c.relname                       AS table_name,
            t.tgname                        AS trigger_name,
            pg_get_triggerdef(t.oid)        AS definition,
            t.tgenabled::TEXT               AS enabled
        FROM pg_trigger   t
        JOIN pg_class     c ON c.oid = t.tgrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE NOT t.tgisinternal
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname, t.tgname
        "#
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn list_views(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                       AS schema_name,
            c.relname                       AS view_name,
            pg_get_viewdef(c.oid, true)     AS definition
        FROM pg_class     c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind = 'v'
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname
        "#
    }

    // ── Materialized views ────────────────────────────────────────────────────

    pub fn list_materialized_views(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                       AS schema_name,
            c.relname                       AS view_name,
            pg_get_viewdef(c.oid, true)     AS definition
        FROM pg_class     c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind = 'm'
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname
        "#
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    pub fn list_functions(&self) -> &'static str {
        r#"
        SELECT
            n.nspname                           AS schema_name,
            p.proname                           AS fn_name,
            l.lanname                           AS language,
            pg_get_function_result(p.oid)       AS return_type,
            pg_get_functiondef(p.oid)           AS definition,
            p.proisagg                          AS is_aggregate,
            CASE p.provolatile
                WHEN 'v' THEN 'VOLATILE'
                WHEN 's' THEN 'STABLE'
                WHEN 'i' THEN 'IMMUTABLE'
            END                                 AS volatility
        FROM pg_proc      p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        JOIN pg_language  l ON l.oid = p.prolang
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND p.prokind = 'f'
        ORDER BY n.nspname, p.proname
        "#
    }

    // ── Procedures — version gated ────────────────────────────────────────────

    pub fn list_procedures(&self) -> Option<&'static str> {
        if !self.caps.procedures {
            return None;  // PostgreSQL < 11 has no stored procedures
        }
        Some(r#"
        SELECT
            n.nspname                           AS schema_name,
            p.proname                           AS proc_name,
            l.lanname                           AS language,
            pg_get_functiondef(p.oid)           AS definition
        FROM pg_proc      p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        JOIN pg_language  l ON l.oid = p.prolang
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND p.prokind = 'p'
        ORDER BY n.nspname, p.proname
        "#)
    }

    // ── Sequences ─────────────────────────────────────────────────────────────

    pub fn list_sequences(&self) -> &'static str {
        r#"
        SELECT
            n.nspname       AS schema_name,
            c.relname       AS seq_name,
            s.seqtypid::regtype::TEXT   AS data_type,
            s.seqstart      AS start_value,
            s.seqmin        AS min_value,
            s.seqmax        AS max_value,
            s.seqincrement  AS increment,
            s.seqcycle      AS cycle
        FROM pg_sequence  s
        JOIN pg_class     c ON c.oid = s.seqrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname
        "#
    }

    // ── Types (enums, composites, domains, ranges) ────────────────────────────

    pub fn list_types(&self) -> &'static str {
        r#"
        SELECT
            n.nspname   AS schema_name,
            t.typname   AS type_name,
            CASE t.typtype
                WHEN 'e' THEN 'enum'
                WHEN 'c' THEN 'composite'
                WHEN 'd' THEN 'domain'
                WHEN 'r' THEN 'range'
                ELSE 'other'
            END         AS type_kind,
            CASE WHEN t.typtype = 'e' THEN
                ARRAY(
                    SELECT e.enumlabel
                    FROM pg_enum e
                    WHERE e.enumtypid = t.oid
                    ORDER BY e.enumsortorder
                )
            ELSE '{}'
            END         AS enum_values
        FROM pg_type      t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE t.typtype IN ('e', 'c', 'd', 'r')
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, t.typname
        "#
    }

    // ── Extensions ────────────────────────────────────────────────────────────

    pub fn list_extensions(&self) -> &'static str {
        r#"
        SELECT
            e.extname       AS name,
            e.extversion    AS version,
            n.nspname       AS schema_name
        FROM pg_extension e
        JOIN pg_namespace n ON n.oid = e.extnamespace
        ORDER BY e.extname
        "#
    }

    // ── Generated columns — version gated ────────────────────────────────────

    pub fn list_generated_columns(&self) -> Option<&'static str> {
        if !self.caps.generated_columns {
            return None;  // PostgreSQL < 12
        }
        Some(r#"
        SELECT
            n.nspname   AS schema_name,
            c.relname   AS table_name,
            a.attname   AS col_name,
            pg_get_expr(d.adbin, d.adrelid) AS generation_expr
        FROM pg_attribute a
        JOIN pg_class     c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_attrdef   d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
        WHERE a.attgenerated != ''
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
        ORDER BY n.nspname, c.relname, a.attnum
        "#)
    }
}