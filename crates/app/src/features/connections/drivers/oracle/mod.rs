use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;
use tokio::task;

use crate::features::connections::{
    domain::Connection,
    drivers::{
        trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
        versions::{ServerVersion, VersionCapabilities},
    },
};

pub mod pool;
pub mod queries;
pub mod schema;

use pool::OraclePool;

pub struct OracleDriver {
    pool: Arc<OraclePool>,
    version: ServerVersion,
    caps: VersionCapabilities,
}

impl OracleDriver {
    pub async fn connect(conn: &Connection) -> Result<Self, String> {
        let host = conn.host.as_deref().unwrap_or("localhost").to_string();
        let port = conn.port.unwrap_or(1521);
        let service = conn.database_name.clone().unwrap_or_else(|| "ORCL".into());
        let username = conn.username.clone().unwrap_or_default();
        let password = conn.password.clone().unwrap_or_default();
        let connect_string = format!("//{host}:{port}/{service}");
        let pool_min = conn.pool_min;
        let pool_max = conn.pool_max;

        let pool = OraclePool::new(username, password, connect_string, pool_min, pool_max)
            .await
            .map_err(|e| e.to_string())?;
        let pool = Arc::new(pool);

        let version = Self::detect_version(Arc::clone(&pool)).await?;
        let caps = VersionCapabilities::for_oracle(&version);

        tracing::info!(
            version = %version.raw,
            major   = version.major,
            has_fetch_first       = caps.fetch_first,
            has_listagg           = caps.listagg,
            has_listagg_overflow  = caps.listagg_overflow,
            has_json_support      = caps.json_support,
            has_packages          = caps.packages,
            "Oracle connected"
        );

        Ok(Self { pool, version, caps })
    }

    async fn detect_version(pool: Arc<OraclePool>) -> Result<ServerVersion, String> {
        let raw = task::spawn_blocking(move || -> Result<String, String> {
            let conn = pool.get().map_err(|e| e.to_string())?;
            let row = conn
                .query_row("SELECT BANNER FROM V$VERSION WHERE ROWNUM = 1", &[])
                .map_err(|e| e.to_string())?;
            row.get::<_, String>(0).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;

        Ok(ServerVersion::parse(&raw))
    }
}

#[async_trait]
impl DatabaseDriver for OracleDriver {
    fn version(&self) -> &ServerVersion {
        &self.version
    }

    async fn ping(&self) -> Result<(String, u64), String> {
        let pool = Arc::clone(&self.pool);
        let start = Instant::now();

        let banner = task::spawn_blocking(move || {
            let conn = pool.get().map_err(|e| e.to_string())?;
            let row = conn
                .query_row("SELECT BANNER FROM V$VERSION WHERE ROWNUM = 1", &[])
                .map_err(|e| e.to_string())?;
            row.get::<_, String>(0).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;

        Ok((banner, start.elapsed().as_millis() as u64))
    }

    async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
        let pool = Arc::clone(&self.pool);
        let caps = self.caps.clone();
        let raw = self.version.raw.clone();

        let (tables, views, mat_views, functions, procedures, sequences, types, packages, jobs) =
            task::spawn_blocking(move || -> Result<_, String> {
                let conn = pool.get().map_err(|e| e.to_string())?;

                let tables = schema::fetch_tables(&conn, &caps)?;
                let views = schema::fetch_views(&conn, &caps)?;
                let mat_views = schema::fetch_materialized_views(&conn, &caps)?;
                let functions = schema::fetch_functions(&conn, &caps)?;
                let procedures = schema::fetch_procedures(&conn, &caps)?;
                let sequences = schema::fetch_sequences(&conn, &caps)?;
                let types = schema::fetch_types(&conn, &caps)?;
                let packages = schema::fetch_packages(&conn, &caps)?;
                let jobs = schema::fetch_jobs(&conn, &caps)?;

                Ok((tables, views, mat_views, functions, procedures, sequences, types, packages, jobs))
            })
            .await
            .map_err(|e| e.to_string())??;

        let schemas = schema::group_by_schema(
            tables, views, mat_views, functions, procedures, sequences, types, packages,
        );

        Ok(DatabaseMetadata::Relational(SchemaInfo {
            database_name: format!("Oracle {} ({})", oracle_version_name(self.version.major), raw),
            schemas,
            scheduled_jobs: jobs,
        }))
    }
}


fn oracle_version_name(major: u32) -> &'static str {
    match major {
        11 => "11g",
        12 => "12c",
        18 => "18c",
        19 => "19c",
        21 => "21c",
        23 => "23ai",
        _  => "unknown",
    }
}

// use async_trait::async_trait;
// use oracle::{
//     Connection as OracleConnection,
// };
// use std::sync::Arc;
// use std::time::Instant;
// use tokio::task;

// use crate::features::connections::{
//     domain::Connection,
//     drivers::{
//         trait_::{DatabaseDriver, DatabaseMetadata, SchemaInfo},
//         versions::{ServerVersion, VersionCapabilities},
//     },
// };

// pub mod queries;
// pub mod schema;
// pub mod pool;

// use pool::OraclePool;

// pub struct OracleDriver {
//     pool: Arc<OraclePool>,
//     version: ServerVersion,
//     caps: VersionCapabilities,
// }

// impl OracleDriver {
//    pub async fn connect(conn: &Connection) -> Result<Self, String> {
//         let host = conn.host.as_deref().unwrap_or("localhost").to_string();
//         let port = conn.port.unwrap_or(1521);
//         let service = conn.database_name.clone().unwrap_or_else(|| "ORCL".into());
//         let username = conn.username.clone().unwrap_or_default();
//         let password = conn.password.clone().unwrap_or_default();
//         let connect_string = format!("//{host}:{port}/{service}");
//         let pool_min = conn.pool_min;
//         let pool_max = conn.pool_max;
 
//         let pool = OraclePool::new(username, password, connect_string, pool_min, pool_max)
//             .await
//             .map_err(|e| e.to_string())?;
//         let pool = Arc::new(pool);
 
//         let version = Self::detect_version(Arc::clone(&pool)).await?;
//         let caps = VersionCapabilities::for_oracle(&version);
 
//         tracing::info!(
//             version = %version.raw,
//             major   = version.major,
//             has_fetch_first       = caps.fetch_first,
//             has_listagg           = caps.listagg,
//             has_listagg_overflow  = caps.listagg_overflow,
//             has_json_support      = caps.json_support,
//             has_packages          = caps.packages,
//             "Oracle connected"
//         );
 
//         Ok(Self { pool, version, caps })
//     }

//     async fn detect_version(pool: Arc<OraclePool>) -> Result<ServerVersion, String> {
//         let mut conn = pool.get().await?;
//         let raw: String = conn
//             .run_blocking(|c: &mut OracleConnection| {
//                 let row = c
//                     .query_row_as::<String>(
//                         "SELECT banner FROM v$version WHERE banner LIKE 'Oracle%'",
//                         &[],
//                     )
//                     .map_err(|e| e.to_string())?;
//                 Ok(row)
//             })
//             .await?;
 
//         Ok(ServerVersion::parse(&raw))
//     }
// }

// // ── DatabaseDriver trait impl ─────────────────────────────────────────────────

// #[async_trait]
// impl DatabaseDriver for OracleDriver {
//     fn version(&self)      -> &ServerVersion      { &self.version }

//     async fn ping(&self) -> Result<(String, u64), String> {
//         let pool  = Arc::clone(&self.pool);
//         let start = Instant::now();

//          let conn = pool.get().await?;

//          let banner: String = conn
//             .run_blocking(|c: &mut OracleConnection| {
//                 let row = c
//                     .query_row_as::<String>(
//                         "SELECT BANNER FROM V$VERSION WHERE ROWNUM = 1",
//                         &[],
//                     )
//                     .map_err(|e| e.to_string())?;
//                 Ok(row)
//             })
//             .await?;

//         Ok((banner, start.elapsed().as_millis() as u64))
//     }

//     async fn fetch_schema(&self) -> Result<DatabaseMetadata, String> {
//         let pool = Arc::clone(&self.pool);
//         let caps = self.caps.clone();
//         let raw  = self.version.raw.clone();

//         // Everything runs inside spawn_blocking because oracle-rs is sync
//         let (tables, views, mat_views, functions, procedures,
//              sequences, types, packages, jobs) =
//             task::spawn_blocking(move || -> Result<_, String> {
//                 let conn = pool.get();

//                 let tables      = schema::fetch_tables(&conn, &caps)?;
//                 let views       = schema::fetch_views(&conn, &caps)?;
//                 let mat_views   = schema::fetch_materialized_views(&conn, &caps)?;
//                 let functions   = schema::fetch_functions(&conn, &caps)?;
//                 let procedures  = schema::fetch_procedures(&conn, &caps)?;
//                 let sequences   = schema::fetch_sequences(&conn, &caps)?;
//                 let types       = schema::fetch_types(&conn, &caps)?;
//                 let packages    = schema::fetch_packages(&conn, &caps)?;
//                 let jobs        = schema::fetch_jobs(&conn, &caps)?;

//                 Ok((tables, views, mat_views, functions, procedures,
//                     sequences, types, packages, jobs))
//             })
//             .await
//             .map_err(|e| e.to_string())??;

//         let schemas = schema::group_by_schema(
//             tables,
//             views,
//             mat_views,
//             functions,
//             procedures,
//             sequences,
//             types,
//             packages,
//         );

//         Ok(DatabaseMetadata::Relational(SchemaInfo {
//             database_name:  format!("Oracle {} ({})",
//                 oracle_version_name(self.version.major),
//                 raw,
//             ),
//             schemas,
//             scheduled_jobs: jobs,
//         }))
//     }
// }

// // ── Version display name ──────────────────────────────────────────────────────
