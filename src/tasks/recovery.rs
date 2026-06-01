use tokio::time::{interval, Duration};
use sqlx::PgPool;
use log::{info, error};
use chrono::Utc;
use uuid::Uuid;
use crate::cache::RedisCache;
use crate::repositories::DeveloperRepository;
use crate::errors::AppError;

pub fn start_recovery_task(pool: PgPool, mut cache: RedisCache) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            match recover_deduction_for_all(&pool, &mut cache).await {
                Ok(count) if count > 0 => {
                    info!("Recovered deduction for {} developers", count);
                }
                Err(e) => {
                    error!("Error recovering deduction: {:?}", e);
                }
                _ => {}
            }
        }
    });
}

async fn recover_deduction_for_all(
    pool: &PgPool,
    cache: &mut RedisCache,
) -> Result<u64, AppError> {
    let dev_repo = DeveloperRepository::new(pool);
    let devs = dev_repo.get_all().await?;

    let mut recovered_count = 0u64;

    for dev in devs {
        let last_recovery = dev.last_recovery_time.unwrap_or_else(|| Utc::now() - chrono::Duration::seconds(86400));
        let interval = chrono::Duration::seconds(dev.recovery_interval_secs as i64);

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
            dev_repo.update_deduction_available(dev.developer_uuid, new_available).await?;
            dev_repo.update_last_recovery_time(dev.developer_uuid).await?;

            info!(
                "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {}, interval: {}s)",
                dev.developer_uuid, current_available, new_available, limit, recovery_amount, dev.recovery_interval_secs
            );
            recovered_count += 1;
        }
    }

    Ok(recovered_count)
}

pub async fn recover_deduction_for_developer(
    pool: &PgPool,
    cache: &mut RedisCache,
    dev_uuid: Uuid,
    recovery_amount: i32,
) -> Result<i32, AppError> {
    let dev_repo = DeveloperRepository::new(pool);
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

    let new_available = std::cmp::min(current_available + recovery_amount, limit);

    cache.set_deduction_data(dev_uuid, new_available, limit).await;
    dev_repo.update_deduction_available(dev_uuid, new_available).await?;

    info!(
        "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {})",
        dev_uuid, current_available, new_available, limit, recovery_amount
    );

    Ok(new_available)
}
