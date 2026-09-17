use crate::features::connections::drivers::versions::VersionCapabilities;

pub struct MySqlQueries<'a> {
    caps: &'a VersionCapabilities,
}

impl<'a> MySqlQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    // ── Tables ────────────────────────────────────────────────────────────────

    pub fn list_tables(&self) -> &'static str {
        r#"
        SELECT
            TABLE_NAME      AS table_name,
            TABLE_COMMENT   AS table_comment,
            TABLE_ROWS      AS row_estimate,
            DATA_LENGTH + INDEX_LENGTH AS size_bytes
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE()
          AND TABLE_TYPE   = 'BASE TABLE'
        ORDER BY TABLE_NAME
        "#
    }

    pub fn list_columns(&self) -> &'static str {
        r#"
        SELECT
            TABLE_NAME,
            COLUMN_NAME,
            DATA_TYPE,
            IS_NULLABLE,
            COLUMN_DEFAULT,
            CHARACTER_MAXIMUM_LENGTH    AS max_length,
            NUMERIC_PRECISION,
            COLUMN_KEY,
            COLUMN_COMMENT,
            GENERATION_EXPRESSION
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, ORDINAL_POSITION
        "#
    }

    // ── Indexes ───────────────────────────────────────────────────────────────
    // GROUP_CONCAT available in all MySQL versions — no branching needed here

    pub fn list_indexes(&self) -> &'static str {
        r#"
        SELECT
            TABLE_NAME,
            INDEX_NAME,
            NON_UNIQUE,
            INDEX_TYPE,
            GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) AS columns
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
        GROUP BY TABLE_NAME, INDEX_NAME, NON_UNIQUE, INDEX_TYPE
        ORDER BY TABLE_NAME, INDEX_NAME
        "#
    }

    // ── Foreign keys ──────────────────────────────────────────────────────────

    pub fn list_foreign_keys(&self) -> &'static str {
        r#"
        SELECT
            k.TABLE_NAME,
            k.COLUMN_NAME,
            k.CONSTRAINT_NAME,
            k.REFERENCED_TABLE_SCHEMA   AS ref_schema,
            k.REFERENCED_TABLE_NAME     AS ref_table,
            k.REFERENCED_COLUMN_NAME    AS ref_col,
            r.DELETE_RULE,
            r.UPDATE_RULE
        FROM information_schema.KEY_COLUMN_USAGE k
        JOIN information_schema.REFERENTIAL_CONSTRAINTS r
            ON  r.CONSTRAINT_NAME   = k.CONSTRAINT_NAME
            AND r.CONSTRAINT_SCHEMA = k.TABLE_SCHEMA
        WHERE k.TABLE_SCHEMA = DATABASE()
          AND k.REFERENCED_TABLE_NAME IS NOT NULL
        ORDER BY k.TABLE_NAME
        "#
    }

    pub fn list_constraints(&self) -> &'static str {
        r#"
        SELECT
            TABLE_NAME,
            CONSTRAINT_NAME,
            CONSTRAINT_TYPE
        FROM information_schema.TABLE_CONSTRAINTS
        WHERE TABLE_SCHEMA     = DATABASE()
          AND CONSTRAINT_TYPE IN ('CHECK', 'UNIQUE')
        ORDER BY TABLE_NAME, CONSTRAINT_NAME
        "#
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn list_views(&self) -> &'static str {
        r#"
        SELECT
            TABLE_NAME      AS view_name,
            VIEW_DEFINITION AS definition,
            IS_UPDATABLE
        FROM information_schema.VIEWS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME
        "#
    }

    // ── Triggers ──────────────────────────────────────────────────────────────

    pub fn list_triggers(&self) -> &'static str {
        r#"
        SELECT
            TRIGGER_NAME,
            EVENT_OBJECT_TABLE  AS table_name,
            ACTION_TIMING       AS timing,
            EVENT_MANIPULATION  AS event,
            ACTION_STATEMENT    AS definition
        FROM information_schema.TRIGGERS
        WHERE TRIGGER_SCHEMA = DATABASE()
        ORDER BY TRIGGER_NAME
        "#
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    pub fn list_functions(&self) -> &'static str {
        r#"
        SELECT
            ROUTINE_NAME        AS fn_name,
            DTD_IDENTIFIER      AS return_type,
            ROUTINE_DEFINITION  AS definition
        FROM information_schema.ROUTINES
        WHERE ROUTINE_SCHEMA = DATABASE()
          AND ROUTINE_TYPE   = 'FUNCTION'
        ORDER BY ROUTINE_NAME
        "#
    }

    // ── Procedures ────────────────────────────────────────────────────────────

    pub fn list_procedures(&self) -> &'static str {
        r#"
        SELECT
            ROUTINE_NAME        AS proc_name,
            ROUTINE_DEFINITION  AS definition
        FROM information_schema.ROUTINES
        WHERE ROUTINE_SCHEMA = DATABASE()
          AND ROUTINE_TYPE   = 'PROCEDURE'
        ORDER BY ROUTINE_NAME
        "#
    }

    pub fn list_procedure_params(&self) -> &'static str {
        r#"
        SELECT
            SPECIFIC_NAME   AS proc_name,
            PARAMETER_NAME  AS param_name,
            DATA_TYPE,
            PARAMETER_MODE  AS mode,
            ORDINAL_POSITION
        FROM information_schema.PARAMETERS
        WHERE SPECIFIC_SCHEMA = DATABASE()
        ORDER BY SPECIFIC_NAME, ORDINAL_POSITION
        "#
    }

    // ── Scheduled events ──────────────────────────────────────────────────────

    pub fn list_events(&self) -> &'static str {
        r#"
        SELECT
            EVENT_NAME,
            STATUS,
            EVENT_DEFINITION    AS definition,
            INTERVAL_VALUE,
            INTERVAL_FIELD,
            LAST_EXECUTED,
            EXECUTE_AT
        FROM information_schema.EVENTS
        WHERE EVENT_SCHEMA = DATABASE()
        ORDER BY EVENT_NAME
        "#
    }

    // ── Window functions — version gated ──────────────────────────────────────

    pub fn list_window_functions_supported(&self) -> bool {
        self.caps.window_functions   // MySQL 8.0+ only
    }

    // ── JSON table — version gated ────────────────────────────────────────────

    pub fn json_table_supported(&self) -> bool {
        self.caps.json_table   // MySQL 8.0+ only
    }

    // ── Generated columns — version gated ────────────────────────────────────

    pub fn list_generated_columns(&self) -> Option<&'static str> {
        if !self.caps.generated_columns {
            return None;  // MySQL < 5.7
        }
        Some(r#"
        SELECT
            TABLE_NAME,
            COLUMN_NAME,
            GENERATION_EXPRESSION,
            EXTRA   AS stored_or_virtual
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA          = DATABASE()
          AND GENERATION_EXPRESSION != ''
        ORDER BY TABLE_NAME, ORDINAL_POSITION
        "#)
    }

    // ── CTE support — version gated ───────────────────────────────────────────

    pub fn cte_supported(&self) -> bool {
        self.caps.cte_update_delete   // MySQL 8.0+
    }

    // ── MariaDB specific ──────────────────────────────────────────────────────
    // Sequence objects only exist in MariaDB, not MySQL

    pub fn list_sequences(&self) -> Option<&'static str> {
        if !self.caps.sequences {
            return None;  // MySQL never — MariaDB 10.3+
        }
        Some(r#"
        SELECT
            SEQUENCE_NAME,
            MINIMUM_VALUE,
            MAXIMUM_VALUE,
            START_VALUE,
            INCREMENT,
            CYCLE_OPTION
        FROM information_schema.SEQUENCES
        WHERE SEQUENCE_SCHEMA = DATABASE()
        ORDER BY SEQUENCE_NAME
        "#)
    }

    // ── Temporal / system-versioned tables (MariaDB 10.3+) ────────────────────

    pub fn list_temporal_tables(&self) -> Option<&'static str> {
        if !self.caps.temporal_tables {
            return None;
        }
        Some(r#"
        SELECT
            TABLE_NAME,
            TABLE_COMMENT
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA  = DATABASE()
          AND TABLE_TYPE    = 'BASE TABLE'
          AND CREATE_OPTIONS LIKE '%WITH SYSTEM VERSIONING%'
        ORDER BY TABLE_NAME
        "#)
    }
}