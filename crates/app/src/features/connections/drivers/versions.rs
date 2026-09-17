use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub raw: String,
}

impl ServerVersion {
    pub fn parse(raw: &str) -> Self {
        let nums: Vec<u32> = raw
            .split(|c: char| !c.is_ascii_digit())
            .filter_map(|s| s.parse().ok())
            .take(3)
            .collect();
        Self {
            major: nums.first().copied().unwrap_or(0),
            minor: nums.get(1).copied().unwrap_or(0),
            patch: nums.get(2).copied().unwrap_or(0),
            raw: raw.to_string(),
        }
    }

    pub fn at_least(&self, major: u32, minor: u32) -> bool {
        (self.major, self.minor) >= (major, minor)
    }
}

#[derive(Debug, Clone, Default)]
pub struct VersionCapabilities {
    // ── Aggregation ───────────────────────────────────────────────────────────
    pub string_agg: bool, // PG: always | SS: 2017+ | MySQL: 8.0+ (GROUP_CONCAT always)
    pub listagg: bool,    // Oracle 11.2+
    pub listagg_overflow: bool, // Oracle 12.2+
    pub group_concat: bool, // MySQL / MariaDB always

    // ── Pagination ────────────────────────────────────────────────────────────
    pub fetch_first: bool,  // Oracle 12c+ / PG always / SS uses TOP
    pub offset_fetch: bool, // SS 2012+ / PG always / MySQL LIMIT OFFSET always

    // ── DDL features ──────────────────────────────────────────────────────────
    pub sequences: bool, // PG always | Oracle always | SS 2012+ | MySQL 8.x (partial)
    pub identity_columns: bool, // Oracle 12c+ | SS always | PG: SERIAL/GENERATED always
    pub generated_columns: bool, // PG 12+ | MySQL 5.7+ | SS 2014+ | Oracle 11g+
    pub drop_if_exists: bool, // PG always | SS 2016+ | MySQL always | Oracle 23ai

    // ── JSON ──────────────────────────────────────────────────────────────────
    pub json_support: bool, // PG 9.2+ | MySQL 5.7+ | SS 2016+ | Oracle 12.2+
    pub json_table: bool,   // PG 10+ (json_to_recordset) | MySQL 8.0+ | Oracle 12.2+

    // ── Window / analytics ────────────────────────────────────────────────────
    pub window_functions: bool, // PG always | SS 2005+ | MySQL 8.0+ | Oracle always
    pub cume_dist: bool,        // PG always | SS 2012+ | MySQL 8.0+ | Oracle always

    // ── Misc ──────────────────────────────────────────────────────────────────
    pub materialized_views: bool, // PG always | Oracle always | SS: indexed views only
    pub lateral_join: bool,       // PG always | Oracle 12c+ | MySQL 8.0+ | SS: CROSS APPLY
    pub cte_update_delete: bool,  // PG always | SS always | MySQL 8.0+ (limited)
    pub returning_clause: bool,   // PG always | Oracle 12c+ (RETURNING INTO) | SS: OUTPUT
    pub procedures: bool,         // All — but syntax varies
    pub packages: bool,           // Oracle only
    pub temporal_tables: bool,    // SS 2016+ | MariaDB 10.3+ (SYSTEM VERSIONED)

    pub redis_acl: bool,     // Redis 6.0+ ACL support
    pub redis_streams: bool, // Redis 5.0+ stream type
    pub redis_lmpop: bool,   // Redis 7.0+ LMPOP command

    // SQLite specific
    pub sqlite_json: bool,       // SQLite 3.38+ JSON functions
    pub sqlite_window_fns: bool, // SQLite 3.25+ window functions
    pub sqlite_generated: bool,  // SQLite 3.31+ generated columns
}

impl VersionCapabilities {
    pub fn for_postgres(v: &ServerVersion) -> Self {
        // PG 12 is our minimum supported version
        Self {
            string_agg: true,
            fetch_first: true,
            offset_fetch: true,
            sequences: true,
            identity_columns: true,
            generated_columns: v.at_least(12, 0),
            drop_if_exists: true,
            json_support: true,
            json_table: true,
            window_functions: true,
            cume_dist: true,
            materialized_views: true,
            lateral_join: true,
            cte_update_delete: true,
            returning_clause: true,
            procedures: v.at_least(14, 0), // proper CALL syntax
            ..Self::default()
        }
    }

    pub fn for_mysql(v: &ServerVersion) -> Self {
        let is_8 = v.major >= 8;
        let is_84 = v.at_least(8, 4);
        Self {
            string_agg: false, // uses GROUP_CONCAT
            group_concat: true,
            fetch_first: false,     // uses LIMIT
            offset_fetch: true,     // LIMIT x OFFSET y
            sequences: false,       // uses AUTO_INCREMENT
            identity_columns: true, // AUTO_INCREMENT
            generated_columns: v.at_least(5, 7),
            drop_if_exists: true,
            json_support: v.at_least(5, 7),
            json_table: is_8,
            window_functions: is_8,
            cume_dist: is_8,
            lateral_join: is_8,
            cte_update_delete: is_8,
            returning_clause: false,
            procedures: true,
            temporal_tables: false,
            ..Self::default()
        }
    }

    pub fn for_mariadb(v: &ServerVersion) -> Self {
        // MariaDB diverged from MySQL — track separately
        let is_10_3 = v.at_least(10, 3);
        let is_10_6 = v.at_least(10, 6);
        let is_11 = v.major >= 11;
        Self {
            string_agg: is_10_3, // GROUP_CONCAT always; string_agg added 10.3.3
            group_concat: true,
            fetch_first: is_10_6,
            offset_fetch: true,
            sequences: is_10_3,
            identity_columns: true,
            generated_columns: v.at_least(5, 2),
            drop_if_exists: true,
            json_support: v.at_least(10, 2),
            json_table: v.at_least(10, 6),
            window_functions: is_10_3,
            cume_dist: is_10_3,
            temporal_tables: is_10_3, // SYSTEM VERSIONED tables
            lateral_join: v.at_least(10, 9),
            procedures: true,
            ..Self::default()
        }
    }

    pub fn for_sqlserver(v: &ServerVersion) -> Self {
        // Internal: 2012=11, 2014=12, 2016=13, 2017=14, 2019=15, 2022=16
        Self {
            string_agg: v.major >= 14,   // 2017+
            offset_fetch: v.major >= 11, // 2012+
            sequences: v.major >= 11,    // 2012+
            identity_columns: true,
            generated_columns: v.major >= 12, // computed columns: always; stored: 2014+
            drop_if_exists: v.major >= 13,    // 2016+
            json_support: v.major >= 13,      // 2016+
            json_table: v.major >= 13,
            window_functions: true,
            cume_dist: v.major >= 11,
            temporal_tables: v.major >= 13, // 2016+
            returning_clause: false,        // uses OUTPUT clause instead
            procedures: true,
            lateral_join: false, // uses CROSS APPLY / OUTER APPLY
            fetch_first: false,  // uses TOP
            ..Self::default()
        }
    }

    pub fn for_oracle(v: &ServerVersion) -> Self {
        // 11=11g, 12=12c, 18=18c, 19=19c, 21=21c, 23=23ai
        Self {
            listagg: v.at_least(11, 2),
            listagg_overflow: v.at_least(12, 2),
            fetch_first: v.major >= 12,
            sequences: true,
            identity_columns: v.major >= 12,
            generated_columns: true,       // virtual columns: always
            drop_if_exists: v.major >= 23, // 23ai
            json_support: v.at_least(12, 2),
            json_table: v.at_least(12, 2),
            window_functions: true,
            cume_dist: true,
            materialized_views: true,
            lateral_join: v.major >= 12,
            cte_update_delete: false,
            returning_clause: true, // RETURNING INTO
            procedures: true,
            packages: true,
            ..Self::default()
        }
    }

    pub fn for_redis(v: &ServerVersion) -> Self {
        Self {
            redis_acl: v.at_least(6, 0),
            redis_streams: v.at_least(5, 0),
            redis_lmpop: v.at_least(7, 0),
            ..Self::default()
        }
    }

    pub fn for_sqlite(v: &ServerVersion) -> Self {
        Self {
            sqlite_json: v.at_least(3, 38),
            sqlite_window_fns: v.at_least(3, 25),
            sqlite_generated: v.at_least(3, 31),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MongoCapabilities {
    pub aggregation: bool,
    pub views: bool,
    pub validators: bool,
    pub transactions: bool,
    pub change_streams: bool,
    pub time_series: bool,
    pub clustered_collections: bool,
    pub column_store_indexes: bool,
    pub wildcard_indexes: bool,
    pub hidden_indexes: bool,
    pub partial_indexes: bool,
    pub ttl_indexes: bool,
    pub search_indexes: bool,
    pub window_functions: bool,
}

impl MongoCapabilities {
    pub fn from_version(version: &ServerVersion) -> Self {
        Self {
            aggregation: true,

            // MongoDB views have existed since 3.4.
            views: version.at_least(3, 4),

            // Collection validators have existed since 3.2.
            validators: version.at_least(3, 2),

            // Multi-document transactions arrived in 4.0
            // for replica sets and later sharded clusters.
            transactions: version.at_least(4, 0),

            // Change streams arrived in 3.6.
            change_streams: version.at_least(3, 6),

            // Time-series collections arrived in 5.0.
            time_series: version.at_least(5, 0),

            // Clustered collections became available in MongoDB 5.3+
            // depending on the exact collection feature being used.
            clustered_collections: version.at_least(5, 3),

            // Columnstore indexes arrived in 7.0.
            column_store_indexes: version.at_least(7, 0),

            // Wildcard indexes arrived in 4.2.
            wildcard_indexes: version.at_least(4, 2),

            // Hidden indexes arrived in 4.4.
            hidden_indexes: version.at_least(4, 4),

            // Partial indexes arrived in 3.2.
            partial_indexes: version.at_least(3, 2),

            // TTL indexes have been supported for a long time.
            ttl_indexes: true,

            // Atlas Search is deployment/service dependent rather
            // than simply a MongoDB server-version feature.
            search_indexes: false,

            // $setWindowFields arrived in 5.0.
            window_functions: version.at_least(5, 0),
        }
    }
}

// ── Oracle version gates ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OracleCapabilities {
    pub has_listagg_overflow: bool, // 12.2+ (ON OVERFLOW TRUNCATE)
    pub has_fetch_first: bool,      // 12c+   (FETCH FIRST N ROWS)
    pub has_approx_functions: bool, // 19c+
    pub has_json_table: bool,       // 12.2+
    pub has_identity_columns: bool, // 12c+
    pub has_lateral_join: bool,     // 12c+
    pub has_listagg: bool,          // 11.2+
}

impl OracleCapabilities {
    pub fn from_version(v: &ServerVersion) -> Self {
        Self {
            has_listagg: v.at_least(11, 2),
            has_listagg_overflow: v.at_least(12, 2),
            has_fetch_first: v.major >= 12,
            has_approx_functions: v.major >= 19,
            has_json_table: v.at_least(12, 2),
            has_identity_columns: v.major >= 12,
            has_lateral_join: v.major >= 12,
        }
    }
}

// ── SQL Server version gates ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SqlServerCapabilities {
    pub has_string_agg: bool,         // 2017+ (v14+)
    pub has_drop_if_exists: bool,     // 2016+ (v13+)
    pub has_sequences: bool,          // 2012+ (v11+)
    pub has_approx_count: bool,       // 2019+ (v15+)
    pub has_json_support: bool,       // 2016+ (v13+)
    pub has_temporal_tables: bool,    // 2016+ (v13+)
    pub has_row_level_security: bool, // 2016+ (v13+)
    pub has_trim: bool,               // 2017+ (v14+)
    pub has_iif: bool,                // 2012+ (v11+)
    pub has_try_parse: bool,          // 2012+ (v11+)
    pub has_offset_fetch: bool,       // 2012+ (v11+)
}

impl SqlServerCapabilities {
    pub fn from_version(v: &ServerVersion) -> Self {
        // SQL Server internal version numbers:
        // 2008 R2 = 10.50, 2012 = 11, 2014 = 12, 2016 = 13,
        // 2017 = 14, 2019 = 15, 2022 = 16
        Self {
            has_iif: v.major >= 11,
            has_try_parse: v.major >= 11,
            has_offset_fetch: v.major >= 11,
            has_sequences: v.major >= 11,
            has_drop_if_exists: v.major >= 13,
            has_json_support: v.major >= 13,
            has_temporal_tables: v.major >= 13,
            has_row_level_security: v.major >= 13,
            has_string_agg: v.major >= 14,
            has_trim: v.major >= 14,
            has_approx_count: v.major >= 15,
        }
    }

    /// Human-readable product name from internal version
    pub fn product_name(v: &ServerVersion) -> &'static str {
        match v.major {
            10 => "SQL Server 2008/2008 R2",
            11 => "SQL Server 2012",
            12 => "SQL Server 2014",
            13 => "SQL Server 2016",
            14 => "SQL Server 2017",
            15 => "SQL Server 2019",
            16 => "SQL Server 2022",
            _ => "SQL Server (unknown version)",
        }
    }
}
