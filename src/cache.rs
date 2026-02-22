use anyhow::Result;
use redis::{aio::ConnectionManager, AsyncCommands};

pub async fn get(conn: &mut ConnectionManager, key: &str) -> Result<Option<String>> {
    let value: Option<String> = conn.get(key).await?;
    Ok(value)
}

pub async fn set_ex(
    conn: &mut ConnectionManager,
    key: &str,
    value: &str,
    ttl_seconds: u64,
) -> Result<()> {
    let _: () = conn.set_ex(key, value, ttl_seconds).await?;
    Ok(())
}
