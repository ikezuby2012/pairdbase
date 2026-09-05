use redis::Client;

pub type RedisClient = Client;

pub fn create_client(url: &str) -> anyhow::Result<RedisClient> {
    let client = Client::open(url)?;

    Ok(client)
}