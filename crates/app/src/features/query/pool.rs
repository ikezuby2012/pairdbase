use dashmap::DashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

use crate::features::connections::domain::{Connection, DbType, SslMode};

pub struct SqlServerManager {
    config: tiberius::Config,
}

impl SqlServerManager {
    fn build(conn: &Connection) -> Result<Self, String> {
        let mut config = tiberius::Config::new();

        config.host(conn.host.as_deref().unwrap_or("localhost"));
        config.port(conn.port.unwrap_or(1433));

        if let Some(db) = &conn.database_name {
            config.database(db);
        }

        match (&conn.username, &conn.password) {
            (Some(u), Some(p)) => {
                config.authentication(tiberius::AuthMethod::sql_server(u, p));
            }
            _ => {
                config.authentication(tiberius::AuthMethod::Integrated);
            }
        }

        let ssl_mode = SslMode::try_from(conn.ssl_mode.as_str()).unwrap_or_default();

        config.encryption(match ssl_mode {
            SslMode::Require | SslMode::VerifyFull | SslMode::VerifyCa => {
                tiberius::EncryptionLevel::Required
            }
            SslMode::Disable => tiberius::EncryptionLevel::NotSupported,
            _ => tiberius::EncryptionLevel::Off,
        });

        config.trust_cert();
        Ok(Self { config })
    }
}

impl bb8::ManageConnection for SqlServerManager {
    type Connection = tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>;
    type Error = tiberius::error::Error;

    // fn connect(
    //     &self,
    // ) -> Pin<Box<dyn Future<Output = Result<Self::Connection, Self::Error>> + Send + '_>> {
    //     let config = self.config.clone();
    //     Box::pin(async move {
    //         use tokio_util::compat::TokioAsyncWriteCompatExt;
    //         let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
    //         tcp.set_nodelay(true)?;
    //         tiberius::Client::connect(config, tcp.compat_write()).await
    //     })
    // }

    fn connect(&self) -> impl Future<Output = Result<Self::Connection, Self::Error>> + Send {
        let config = self.config.clone();

        async move {
            use tokio_util::compat::TokioAsyncWriteCompatExt;

            let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;

            tcp.set_nodelay(true)?;

            tiberius::Client::connect(config, tcp.compat_write()).await
        }
    }

    fn is_valid(
        &self,
        conn: &mut Self::Connection,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move {
            conn.simple_query("SELECT 1").await?.into_results().await?;

            Ok(())
        }
    }

    fn has_broken(&self, _: &mut Self::Connection) -> bool {
        false
    }
}

pub enum ExecutionPool {
    Postgres(sqlx::PgPool),
    MySQL(sqlx::MySqlPool),
    SQLite(sqlx::SqlitePool),
    Oracle(Arc<oracle::pool::Pool>),
    SQLServer(bb8::Pool<SqlServerManager>),
    MongoDB(mongodb::Client),
    Redis(redis::Client),
}

pub struct ExecutionPoolRegistry {
    pools: DashMap<Uuid, Arc<ExecutionPool>>,
}

impl ExecutionPoolRegistry {
    pub fn new() -> Self {
        Self {
            pools: DashMap::new(),
        }
    }

    pub fn get(&self, id: Uuid) -> Option<Arc<ExecutionPool>> {
        self.pools.get(&id).map(|p| Arc::clone(p.value()))
    }

    pub fn register(&self, id: Uuid, pool: ExecutionPool) {
        self.pools.insert(id, Arc::new(pool));
    }

    pub fn remove(&self, id: Uuid) {
        self.pools.remove(&id);
    }

    pub fn is_connected(&self, id: Uuid) -> bool {
        self.pools.contains_key(&id)
    }
}

pub async fn build_execution_pool(conn: &Connection) -> Result<ExecutionPool, String> {
    match conn.db_type {
        DbType::PostgreSQL => build_postgres(conn).await,
        DbType::MySQL => build_mysql(conn).await,
        DbType::SQLite => build_sqlite(conn).await,
        DbType::Oracle => build_oracle(conn).await,
        DbType::SqlServer => build_sqlserver(conn).await,
        DbType::MongoDB => build_mongodb(conn).await,
        DbType::Redis => build_redis(conn),
    }
}

async fn build_postgres(conn: &Connection) -> Result<ExecutionPool, String> {
    let url = format!(
        "postgresql://{}:{}@{}:{}/{}?sslmode={}",
        conn.username.as_deref().unwrap_or("postgres"),
        conn.password.as_deref().unwrap_or(""),
        conn.host.as_deref().unwrap_or("localhost"),
        conn.port.unwrap_or(5432),
        conn.database_name.as_deref().unwrap_or("postgres"),
        conn.ssl_mode.as_str(),
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(conn.pool_min)
        .max_connections(conn.pool_max)
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ExecutionPool::Postgres(pool))
}

async fn build_mysql(conn: &Connection) -> Result<ExecutionPool, String> {
    let url = format!(
        "mysql://{}:{}@{}:{}/{}",
        conn.username.as_deref().unwrap_or("root"),
        conn.password.as_deref().unwrap_or(""),
        conn.host.as_deref().unwrap_or("localhost"),
        conn.port.unwrap_or(3306),
        conn.database_name.as_deref().unwrap_or(""),
    );
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .min_connections(conn.pool_min)
        .max_connections(conn.pool_max)
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ExecutionPool::MySQL(pool))
}

async fn build_sqlite(conn: &Connection) -> Result<ExecutionPool, String> {
    let path = conn.database_name.as_deref().unwrap_or(":memory:");
    let url = if path == ":memory:" {
        "sqlite::memory:".into()
    } else {
        format!("sqlite:{}", path)
    };
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ExecutionPool::SQLite(pool))
}

async fn build_oracle(conn: &Connection) -> Result<ExecutionPool, String> {
    use tokio::task;

    let host = conn
        .host
        .as_deref()
        .unwrap_or("localhost")
        .to_string();

    let port = conn.port.unwrap_or(1521);

    let service = conn
        .database_name
        .clone()
        .unwrap_or_else(|| "ORCL".to_string());

    let username = conn
        .username
        .clone()
        .unwrap_or_default();

    let password = conn
        .password
        .clone()
        .unwrap_or_default();

    let pool_max = conn.pool_max;

    let connect_string = format!(
        "//{host}:{port}/{service}"
    );

    let pool = task::spawn_blocking(move || {
        oracle::pool::PoolBuilder::new(
            &username,
            &password,
            &connect_string,
        )
        .max_connections(pool_max)
        .build()
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(ExecutionPool::Oracle(Arc::new(pool)))
}

async fn build_sqlserver(conn: &Connection) -> Result<ExecutionPool, String> {
    let manager = SqlServerManager::build(conn)?;
    let pool = bb8::Pool::builder()
        .min_idle(Some(conn.pool_min))
        .max_size(conn.pool_max)
        .build(manager)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ExecutionPool::SQLServer(pool))
}

async fn build_mongodb(conn: &Connection) -> Result<ExecutionPool, String> {
    let host = conn.host.as_deref().unwrap_or("localhost");
    let port = conn.port.unwrap_or(27017);
    let db_name = conn.database_name.clone().unwrap_or_else(|| "test".into());
    let url = match (&conn.username, &conn.password) {
        (Some(u), Some(p)) => format!("mongodb://{u}:{p}@{host}:{port}/{db_name}"),
        _ => format!("mongodb://{host}:{port}/{db_name}"),
    };
    let opts = mongodb::options::ClientOptions::parse(&url)
        .await
        .map_err(|e| e.to_string())?;
    let client = mongodb::Client::with_options(opts).map_err(|e| e.to_string())?;
    Ok(ExecutionPool::MongoDB(client))
}

fn build_redis(conn: &Connection) -> Result<ExecutionPool, String> {
    let host = conn.host.as_deref().unwrap_or("localhost");
    let port = conn.port.unwrap_or(6379);
    let url = match &conn.password {
        Some(p) => format!("redis://:{p}@{host}:{port}"),
        None => format!("redis://{host}:{port}"),
    };
    let client = redis::Client::open(url).map_err(|e| e.to_string())?;
    Ok(ExecutionPool::Redis(client))
}
