use redis::aio::ConnectionManager;
use std::time::Duration;

pub async fn create_connection_manager(config: &crate::config::Config) -> Result<ConnectionManager, redis::RedisError> {
    let client = redis::Client::open(config.redis_url.as_str())?;
    let manager = tokio::time::timeout(
        Duration::from_secs(10),
        ConnectionManager::new(client),
    ).await
        .map_err(|_| redis::RedisError::from((
            redis::ErrorKind::IoError,
            "Redis connection timed out after 10s",
        )))?
        .map_err(|e| {
            log::error!("Redis connection failed: {}", e);
            e
        })?;
    log::info!("Redis connection manager established");
    Ok(manager)
}