use crate::features::connections::drivers::versions::VersionCapabilities;

pub struct OracleQueries<'a> {
    caps: &'a VersionCapabilities,
}

impl<'a> OracleQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    pub fn list_tables(&self) -> &'static str {
        r#"
        SELECT
            t.OWNER,
            t.TABLE_NAME,
            c.COMMENTS      AS table_comment,
            t.NUM_ROWS      AS row_estimate
        FROM ALL_TABLES t
        LEFT JOIN ALL_TAB_COMMENTS c
            ON c.OWNER = t.OWNER AND c.TABLE_NAME = t.TABLE_NAME
        WHERE t.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS','DVSYS','LBACSYS',
            'MDSYS','OLAPSYS','ORDPLUGINS','ORDSYS'
        )
        ORDER BY t.OWNER, t.TABLE_NAME
        "#
    }

    pub fn list_columns(&self) -> &'static str {
        r#"
        SELECT
            c.OWNER,
            c.TABLE_NAME,
            c.COLUMN_NAME,
            c.DATA_TYPE,
            c.NULLABLE,
            c.DATA_DEFAULT,
            c.CHAR_LENGTH        AS max_length,
            c.DATA_PRECISION     AS numeric_precision,
            cc.COMMENTS          AS col_comment
        FROM ALL_TAB_COLUMNS c
        LEFT JOIN ALL_COL_COMMENTS cc
            ON  cc.OWNER       = c.OWNER
            AND cc.TABLE_NAME  = c.TABLE_NAME
            AND cc.COLUMN_NAME = c.COLUMN_NAME
        WHERE c.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS','DVSYS','LBACSYS',
            'MDSYS','OLAPSYS','ORDPLUGINS','ORDSYS'
        )
        ORDER BY c.OWNER, c.TABLE_NAME, c.COLUMN_ID
        "#
    }

    pub fn list_indexes(&self) -> &'static str {
        if self.caps.listagg {
            r#"
            SELECT
                i.OWNER,
                i.TABLE_NAME,
                i.INDEX_NAME,
                i.UNIQUENESS,
                i.INDEX_TYPE,
                LISTAGG(ic.COLUMN_NAME, ',')
                    WITHIN GROUP (ORDER BY ic.COLUMN_POSITION) AS columns
            FROM ALL_INDEXES i
            JOIN ALL_IND_COLUMNS ic
                ON ic.INDEX_OWNER = i.OWNER
               AND ic.INDEX_NAME  = i.INDEX_NAME
            WHERE i.OWNER NOT IN (
                'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
                'EXFSYS','CTXSYS','XDB','ANONYMOUS'
            )
            GROUP BY i.OWNER, i.TABLE_NAME, i.INDEX_NAME,
                     i.UNIQUENESS, i.INDEX_TYPE
            ORDER BY i.OWNER, i.TABLE_NAME
            "#
        } else {
            // Oracle < 11.2
            r#"
            SELECT
                i.OWNER,
                i.TABLE_NAME,
                i.INDEX_NAME,
                i.UNIQUENESS,
                i.INDEX_TYPE,
                WM_CONCAT(ic.COLUMN_NAME) AS columns
            FROM ALL_INDEXES i
            JOIN ALL_IND_COLUMNS ic
                ON ic.INDEX_OWNER = i.OWNER
               AND ic.INDEX_NAME  = i.INDEX_NAME
            WHERE i.OWNER NOT IN (
                'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
                'EXFSYS','CTXSYS','XDB','ANONYMOUS'
            )
            GROUP BY i.OWNER, i.TABLE_NAME, i.INDEX_NAME,
                     i.UNIQUENESS, i.INDEX_TYPE
            ORDER BY i.OWNER, i.TABLE_NAME
            "#
        }
    }

    // ── Foreign keys ──────────────────────────────────────────────────────────

    pub fn list_foreign_keys(&self) -> &'static str {
        r#"
        SELECT
            ac.OWNER,
            acc.TABLE_NAME,
            ac.CONSTRAINT_NAME,
            acc.COLUMN_NAME,
            rc.OWNER        AS ref_schema,
            rcc.TABLE_NAME  AS ref_table,
            rcc.COLUMN_NAME AS ref_col,
            ac.DELETE_RULE
        FROM ALL_CONSTRAINTS ac
        JOIN ALL_CONS_COLUMNS acc
            ON acc.CONSTRAINT_NAME = ac.CONSTRAINT_NAME
           AND acc.OWNER           = ac.OWNER
        JOIN ALL_CONSTRAINTS rc
            ON rc.CONSTRAINT_NAME  = ac.R_CONSTRAINT_NAME
           AND rc.OWNER            = ac.R_OWNER
        JOIN ALL_CONS_COLUMNS rcc
            ON rcc.CONSTRAINT_NAME = rc.CONSTRAINT_NAME
           AND rcc.OWNER           = rc.OWNER
           AND rcc.POSITION        = acc.POSITION
        WHERE ac.CONSTRAINT_TYPE = 'R'
          AND ac.OWNER NOT IN (
              'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
              'EXFSYS','CTXSYS','XDB','ANONYMOUS'
          )
        ORDER BY ac.OWNER, acc.TABLE_NAME
        "#
    }

    pub fn list_constraints(&self) -> &'static str {
        r#"
        SELECT
            ac.OWNER,
            ac.TABLE_NAME,
            ac.CONSTRAINT_NAME,
            ac.CONSTRAINT_TYPE,
            ac.SEARCH_CONDITION
        FROM ALL_CONSTRAINTS ac
        WHERE ac.CONSTRAINT_TYPE IN ('C', 'U')
          AND ac.OWNER NOT IN (
              'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
              'EXFSYS','CTXSYS','XDB','ANONYMOUS'
          )
        ORDER BY ac.OWNER, ac.TABLE_NAME
        "#
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn list_views(&self) -> &'static str {
        r#"
        SELECT
            v.OWNER,
            v.VIEW_NAME,
            v.TEXT
        FROM ALL_VIEWS v
        WHERE v.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS'
        )
        ORDER BY v.OWNER, v.VIEW_NAME
        "#
    }

    pub fn list_materialized_views(&self) -> &'static str {
        r#"
        SELECT
            m.OWNER,
            m.MVIEW_NAME,
            m.QUERY,
            m.REFRESH_MODE,
            m.LAST_REFRESH_DATE
        FROM ALL_MVIEWS m
        WHERE m.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS'
        )
        ORDER BY m.OWNER, m.MVIEW_NAME
        "#
    }

    // ── Triggers ──────────────────────────────────────────────────────────────

    pub fn list_triggers(&self) -> &'static str {
        r#"
        SELECT
            t.OWNER,
            t.TABLE_NAME,
            t.TRIGGER_NAME,
            t.TRIGGER_TYPE,
            t.TRIGGERING_EVENT,
            t.TRIGGER_BODY,
            t.STATUS
        FROM ALL_TRIGGERS t
        WHERE t.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS'
        )
        ORDER BY t.OWNER, t.TABLE_NAME, t.TRIGGER_NAME
        "#
    }

    pub fn list_functions(&self) -> &'static str {
        r#"
        SELECT OWNER, NAME, TEXT
        FROM ALL_SOURCE
        WHERE TYPE = 'FUNCTION'
          AND OWNER NOT IN (
              'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
              'EXFSYS','CTXSYS','XDB','ANONYMOUS'
          )
        ORDER BY OWNER, NAME, LINE
        "#
    }

    // ── Procedures ────────────────────────────────────────────────────────────

    pub fn list_procedures(&self) -> &'static str {
        r#"
        SELECT OWNER, NAME, TEXT
        FROM ALL_SOURCE
        WHERE TYPE = 'PROCEDURE'
          AND OWNER NOT IN (
              'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
              'EXFSYS','CTXSYS','XDB','ANONYMOUS'
          )
        ORDER BY OWNER, NAME, LINE
        "#
    }

    pub fn list_procedure_args(&self) -> &'static str {
        r#"
        SELECT
            OWNER,
            OBJECT_NAME,
            ARGUMENT_NAME,
            DATA_TYPE,
            IN_OUT,
            DEFAULT_VALUE
        FROM ALL_ARGUMENTS
        WHERE OBJECT_TYPE = 'PROCEDURE'
          AND OWNER NOT IN ('SYS','SYSTEM','OUTLN','DBSNMP')
        ORDER BY OWNER, OBJECT_NAME, POSITION
        "#
    }

    // ── Sequences ─────────────────────────────────────────────────────────────

    pub fn list_sequences(&self) -> &'static str {
        r#"
        SELECT
            SEQUENCE_OWNER,
            SEQUENCE_NAME,
            MIN_VALUE,
            MAX_VALUE,
            INCREMENT_BY,
            CYCLE_FLAG,
            LAST_NUMBER
        FROM ALL_SEQUENCES
        WHERE SEQUENCE_OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS'
        )
        ORDER BY SEQUENCE_OWNER, SEQUENCE_NAME
        "#
    }

    pub fn list_types(&self) -> &'static str {
        r#"
        SELECT
            t.OWNER,
            t.TYPE_NAME,
            t.TYPECODE
        FROM ALL_TYPES t
        WHERE t.OWNER NOT IN (
            'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
            'EXFSYS','CTXSYS','XDB','ANONYMOUS'
        )
        ORDER BY t.OWNER, t.TYPE_NAME
        "#
    }

    // ── Packages (Oracle only) ────────────────────────────────────────────────

    pub fn list_packages(&self) -> &'static str {
        r#"
        SELECT OWNER, NAME, TYPE, TEXT
        FROM ALL_SOURCE
        WHERE TYPE IN ('PACKAGE', 'PACKAGE BODY')
          AND OWNER NOT IN (
              'SYS','SYSTEM','OUTLN','DBSNMP','APPQOSSYS','WMSYS',
              'EXFSYS','CTXSYS','XDB','ANONYMOUS'
          )
        ORDER BY OWNER, NAME, TYPE, LINE
        "#
    }

    // ── Scheduled jobs ────────────────────────────────────────────────────────

    pub fn list_jobs(&self) -> &'static str {
        r#"
        SELECT
            JOB_NAME,
            OWNER,
            ENABLED,
            REPEAT_INTERVAL,
            STATE,
            JOB_ACTION
        FROM ALL_SCHEDULER_JOBS
        WHERE OWNER NOT IN ('SYS','SYSTEM','DBSNMP','WMSYS','EXFSYS')
        ORDER BY OWNER, JOB_NAME
        "#
    }

    // ── Version-gated helpers ─────────────────────────────────────────────────

    /// Oracle 12c+ pagination — caller falls back to ROWNUM if false
    pub fn supports_fetch_first(&self) -> bool {
        self.caps.fetch_first
    }

    pub fn paginate(&self, inner: &str, limit: u64) -> String {
        if self.caps.fetch_first {
            format!("{} FETCH FIRST {} ROWS ONLY", inner, limit)
        } else {
            format!("SELECT * FROM ({}) WHERE ROWNUM <= {}", inner, limit)
        }
    }
}
