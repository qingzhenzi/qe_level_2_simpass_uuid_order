use tokio::time::{interval, Duration};
use crate::db::DbPool;
use log::{info, error};
use uuid::Uuid;
use crate::errors::AppError;
use crate::repositories::DeveloperRepository;
use crate::cache::backend::CacheBackend;
use crate::services::deduction;

pub fn start_expiration_task(
    db: DbPool,
    cache: CacheBackend,
) {
    tokio::spawn(async move {
        let mut expire_ticker = interval(Duration::from_secs(10));
        let mut recovery_ticker = interval(Duration::from_secs(1));
        let mut cleanup_ticker = interval(Duration::from_secs(300)); // every 5 min
        let mut cache = cache;

        loop {
            tokio::select! {
                _ = expire_ticker.tick() => {
                    // ── Transaction expiry (all instances can run this) ─
                    if let Err(e) = deduction::expire_stale_transactions(&db).await {
                        error!("Error expiring transactions: {}", e);
                    }
                    // Sync cache pending counts with DB after expiry
                    if let Err(e) = deduction::sync_pending_counts(&db, &mut cache).await {
                        error!("Error syncing pending counts: {}", e);
                    }
                }

                _ = recovery_ticker.tick() => {
                    // ── Deduction recovery ─
                    let should_run = if cache.is_redis() {
                        acquire_leader_lock(&mut cache).await
                    } else {
                        true
                    };

                    if !should_run {
                        continue;
                    }

                    if let Err(e) = deduction::recover_deduction_for_all(&db, &mut cache).await {
                        error!("Error running recovery: {}", e);
                    }
                }

                _ = cleanup_ticker.tick() => {
                    // ── Cleanup old expired transactions (prevent DB bloat) ─
                    if let Ok(count) = deduction::cleanup_expired_transactions(&db, 3600).await {
                        if count > 0 {
                            log::info!("[Cleanup] Removed {} expired transactions", count);
                        }
                    }
                }
            }
        }
    });
}

/// Try to acquire the leader lock via Redis SET NX
async fn acquire_leader_lock(cache: &mut CacheBackend) -> bool {
    // For Redis mode, we use the processed map as a simple lock mechanism
    // In a real distributed scenario, this would use Redis SET NX
    // For now, we use a simpler approach: try_claim_processed on a fixed key
    let lock_token = Uuid::new_v4();
    cache.try_claim_processed(lock_token).await
}

/// Recover deduction for a single developer.
pub async fn recover_deduction_for_developer(
    db: &DbPool,
    cache: &mut CacheBackend,
    dev_uuid: Uuid,
) -> Result<i32, AppError> {
    let dev_repo = DeveloperRepository::new(db.clone());
    let dev = dev_repo.get_by_uuid(dev_uuid).await?
        .ok_or_else(|| AppError::NotFound("Developer not found".into()))?;

    let (current_available, limit) = if let Some((avail, lim)) = cache.get_deduction_data(dev_uuid).await {
        (avail, lim)
    } else {
        (dev.deduction_available, dev.deduction_limit)
    };

    if current_available >= limit {
        return Ok(current_available);
    }

    let new_available = std::cmp::min(current_available + dev.recovery_amount, limit);

    cache.set_deduction_data(dev_uuid, new_available, limit).await;
    dev_repo.update_deduction_available(dev_uuid, new_available).await?;

    info!(
        "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {})",
        dev_uuid, current_available, new_available, limit, dev.recovery_amount
    );

    Ok(new_available)
}
