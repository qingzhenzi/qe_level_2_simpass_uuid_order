use tokio::time::{interval, Duration};
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use log::{info, error};

const LOCK_EXPIRE_TICK: u64 = 15;   // lock TTL for each leader-check cycle

/// Try to acquire a distributed lock via Redis `SET NX`.
/// Without Redis, always returns `true` (each instance runs the task — degraded).
async fn try_acquire_lock(conn: &mut Option<ConnectionManager>, key: &str, ttl_secs: u64) -> bool {
    let Some(ref mut conn) = conn else { return true };
    use redis::AsyncCommands;
    match conn.set_nx(key, "1").await {
        Ok(true) => {
            let _: Result<(), _> = conn.expire(key, ttl_secs as i64).await;
            true
        }
        _ => false,
    }
}

/// Refresh (extend TTL on) the lock we already hold.
async fn refresh_lock(conn: &mut Option<ConnectionManager>, key: &str, ttl_secs: u64) {
    let Some(ref mut conn) = conn else { return };
    use redis::AsyncCommands;
    let _: Result<(), _> = conn.expire(key, ttl_secs as i64).await;
}

pub fn start_expiration_task(
    pool: PgPool,
    redis_conn: Option<ConnectionManager>,
    redis_prefix: String,
) {
    tokio::spawn(async move {
        let mut expire_ticker = interval(Duration::from_secs(10));
        let mut recovery_ticker = interval(Duration::from_secs(1));
        let mut redis_conn = redis_conn;
        let lock_key = format!("{}:leader:expiration", redis_prefix);

        // Track whether WE are the current leader for recovery tasks
        let mut we_are_leader = false;

        loop {
            tokio::select! {
                _ = expire_ticker.tick() => {
                    // ── Transaction expiry ─────────────────────────────────
                    // Every instance can safely expire stale transactions
                    // because the SQL UPDATE is idempotent.
                    match sqlx::query(
                        "UPDATE deduction_transactions SET status = 'expired'
                         WHERE status = 'pending' AND expires_at < NOW()"
                    )
                    .execute(&pool)
                    .await
                    {
                        Ok(r) if r.rows_affected() > 0 => {
                            info!("Expired {} stale transactions", r.rows_affected());
                        }
                        Err(e) => error!("Error expiring transactions: {}", e),
                        _ => {}
                    }
                }

                _ = recovery_ticker.tick() => {
                    // ── Deduction recovery (leader only) ───────────────────
                    // Only one instance should perform recovery to avoid
                    // redundant UPDATEs and last_recovery_time races.

                    if !we_are_leader {
                        we_are_leader = try_acquire_lock(
                            &mut redis_conn,
                            &lock_key,
                            LOCK_EXPIRE_TICK,
                        ).await;
                        if !we_are_leader {
                            // Another instance is the leader — skip this cycle
                            continue;
                        }
                    }

                    // We are (still) the leader — refresh the lock and do recovery
                    refresh_lock(&mut redis_conn, &lock_key, LOCK_EXPIRE_TICK).await;

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

                    for dev in &devs {
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
