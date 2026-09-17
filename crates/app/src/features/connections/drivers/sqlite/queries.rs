use crate::features::connections::drivers::versions::VersionCapabilities;

/// SQLite catalog queries — all read from sqlite_master / pragma commands.
/// No version branching needed — SQLite pragma interface is stable.
pub struct SqliteQueries<'a> {
    #[allow(dead_code)]
    caps: &'a VersionCapabilities,
}

impl<'a> SqliteQueries<'a> {
    pub fn new(caps: &'a VersionCapabilities) -> Self {
        Self { caps }
    }

    /// All tables in the database
    pub fn list_tables(&self) -> &'static str {
        r#"
        SELECT
            name        AS table_name,
            sql         AS ddl
        FROM sqlite_master
        WHERE type = 'table'
          AND name NOT LIKE 'sqlite_%'
        ORDER BY name
        "#
    }

    /// All columns for a specific table via PRAGMA
    /// Called once per table — SQLite has no information_schema
    pub fn table_info(&self, table: &str) -> String {
        format!("PRAGMA table_info('{}')", table)
    }

    /// All indexes
    pub fn list_indexes(&self) -> &'static str {
        r#"
        SELECT
            name        AS index_name,
            tbl_name    AS table_name,
            sql         AS ddl
        FROM sqlite_master
        WHERE type = 'index'
          AND name NOT LIKE 'sqlite_%'
          AND sql IS NOT NULL
        ORDER BY tbl_name, name
        "#
    }

    /// Index details for a specific index
    pub fn index_info(&self, index: &str) -> String {
        format!("PRAGMA index_info('{}')", index)
    }

    /// Foreign keys for a specific table
    pub fn foreign_keys(&self, table: &str) -> String {
        format!("PRAGMA foreign_key_list('{}')", table)
    }

    /// All views
    pub fn list_views(&self) -> &'static str {
        r#"
        SELECT
            name    AS view_name,
            sql     AS definition
        FROM sqlite_master
        WHERE type = 'view'
        ORDER BY name
        "#
    }

    /// All triggers
    pub fn list_triggers(&self) -> &'static str {
        r#"
        SELECT
            name        AS trigger_name,
            tbl_name    AS table_name,
            sql         AS definition
        FROM sqlite_master
        WHERE type = 'trigger'
        ORDER BY tbl_name, name
        "#
    }

    /// SQLite version
    pub fn version(&self) -> &'static str {
        "SELECT sqlite_version()"
    }

    /// Database page size and page count — gives approximate file size
    pub fn page_stats(&self) -> &'static str {
        "PRAGMA page_count; PRAGMA page_size"
    }

    /// WAL mode check
    pub fn journal_mode(&self) -> &'static str {
        "PRAGMA journal_mode"
    }
}