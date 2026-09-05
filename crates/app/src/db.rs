use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub type DbPool = PgPool;

pub async fn create_pool(url: &str, max_conns: u32, min_conns: u32) -> anyhow::Result<DbPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_conns)
        .min_connections(min_conns)
        .connect(url)
        .await?;
    Ok(pool)
}