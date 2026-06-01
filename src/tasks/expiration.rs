use tokio::time::{interval, Duration};
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use log::{info, error};
use crate::repositories::TransactionRepository;

pub fn start_expiration_task(pool: PgPool, redis_conn: Option<ConnectionManager>, redis_prefix: String) {
    tokio::spawn(async move {
        let mut expire_ticker = interval(Duration::from_secs(10));
        let mut recovery_ticker = interval(Duration::from_secs(1));
        let mut redis_conn = redis_conn;

        loop {
            tokio::select! {
                _ = expire_ticker.tick() => {
                    let repo = TransactionRepository::new(&pool);
                    match repo.expire_stale().await {
                        Ok(count) if count > 0 => {
                            info!("Expired {} stale transactions", count);
                        }
                        Err(e) => {
                            error!("Error expiring transactions: {}", e);
                        }
                        _ => {}
                    }
                }
                _ = recovery_ticker.tick() => {
                    use crate::repositories::DeveloperRepository;
                    use crate::cache::RedisCache;

                    let dev_repo = DeveloperRepository::new(&pool);
                    let devs = match dev_repo.get_all().await {
                        Ok(d) => d,
                        Err(e) => {
                            error!("Error getting developers for recovery: {}", e);
                            continue;
                        }
                    };

                    let mut cache = RedisCache::new(redis_conn.clone(), redis_prefix.clone());

                    for dev in devs {
                        use chrono::{Duration as ChronoDuration, Utc};

                        let last_recovery = dev.last_recovery_time.unwrap_or_else(|| Utc::now() - ChronoDuration::seconds(86400));
                        let interval = ChronoDuration::seconds(dev.recovery_interval_secs as i64);

                        if Utc::now() - last_recovery < interval {
                            continue;
                        }

                        let (current_available, limit) = if let Some((avail, lim)) = cache.get_deduction_data(dev.developer_uuid).await {
                            (avail, lim)
                        } else {
                            (dev.deduction_available, dev.deduction_limit)
                        };

                        if current_available >= limit {
                            continue;
                        }

                        let recovery_amount = dev.recovery_amount;
                        let new_available = std::cmp::min(current_available + recovery_amount, limit);

                        if new_available != current_available {
                            cache.set_deduction_data(dev.developer_uuid, new_available, limit).await;
                            if let Err(e) = dev_repo.update_deduction_available(dev.developer_uuid, new_available).await {
                                error!("Error updating deduction available: {}", e);
                            }
                            if let Err(e) = dev_repo.update_last_recovery_time(dev.developer_uuid).await {
                                error!("Error updating last recovery time: {}", e);
                            }

                            info!(
                                "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {}, interval: {}s)",
                                dev.developer_uuid, current_available, new_available, limit, recovery_amount, dev.recovery_interval_secs
                            );
                        }
                    }
                }
            }
        }
    });
}