use chrono::{Duration, Utc};
use uuid::Uuid;
use crate::db::DbPool;
use crate::errors::AppError;
use crate::models::{
    DeductionTransaction, InitiateDeductionResponse,
    ConfirmDeductionRequest, CancelDeductionRequest, Developer,
};
use crate::cache::backend::CacheBackend;

async fn get_or_sync_deduction(
    cache: &mut CacheBackend,
    db: &DbPool,
    dev_uuid: Uuid,
) -> Result<(i32, i32), AppError> {
    if let Some((avail, lim)) = cache.get_deduction_data(dev_uuid).await {
        return Ok((avail, lim));
    }

    let dev = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, Developer>(
                "SELECT * FROM developers WHERE developer_uuid = $1"
            )
            .bind(dev_uuid)
            .fetch_optional(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, Developer>(
                "SELECT * FROM developers WHERE developer_uuid = $1"
            )
            .bind(dev_uuid.to_string())
            .fetch_optional(sq)
            .await?
        }
    }
    .ok_or_else(|| AppError::NotFound("Developer not found".into()))?;

    cache.set_deduction_data(dev_uuid, dev.deduction_available, dev.deduction_limit).await;
    log::info!("Synced deduction data to cache for dev: {}", dev_uuid);

    Ok((dev.deduction_available, dev.deduction_limit))
}

async fn sync_deduction_to_db(
    db: &DbPool,
    dev_uuid: Uuid,
    new_available: i32,
) -> Result<(), AppError> {
    match db {
        DbPool::Postgres(pg) => {
            sqlx::query(
                "UPDATE developers SET deduction_available = $1, updated_at = NOW() WHERE developer_uuid = $2"
            )
            .bind(new_available)
            .bind(dev_uuid)
            .execute(pg)
            .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE developers SET deduction_available = $1, updated_at = $2 WHERE developer_uuid = $3"
            )
            .bind(new_available)
            .bind(Utc::now().to_rfc3339())
            .bind(dev_uuid.to_string())
            .execute(sq)
            .await?;
        }
    }

    log::info!("Synced deduction_available={} to DB for dev {}", new_available, dev_uuid);
    Ok(())
}

pub async fn initiate_deduction(
    db: &DbPool,
    cache: &mut CacheBackend,
    developer_uuid: Uuid,
    amount: i32,
    timeout_secs: u64,
) -> Result<InitiateDeductionResponse, AppError> {
    let (available, limit) = get_or_sync_deduction(cache, db, developer_uuid).await?;

    let pending_amount = cache.get_pending_count(developer_uuid).await as i32;

    if available - pending_amount < amount {
        return Err(AppError::BadRequest(format!(
            "Insufficient deduction available: available={}, pending={}, requested={}",
            available, pending_amount, amount
        )));
    }

    let transaction_token = Uuid::new_v4();
    let commit_token = Uuid::new_v4();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(timeout_secs as i64);

    match db {
        DbPool::Postgres(pg) => {
            sqlx::query(
                r#"INSERT INTO deduction_transactions
                   (developer_uuid, transaction_token, amount, status, expires_at, commit_token)
                   VALUES ($1, $2, $3, 'pending', $4, $5)"#
            )
            .bind(developer_uuid)
            .bind(transaction_token)
            .bind(amount)
            .bind(expires_at)
            .bind(commit_token)
            .execute(pg)
            .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query(
                r#"INSERT INTO deduction_transactions
                   (developer_uuid, transaction_token, amount, status, expires_at, commit_token)
                   VALUES ($1, $2, $3, 'pending', $4, $5)"#
            )
            .bind(developer_uuid.to_string())
            .bind(transaction_token.to_string())
            .bind(amount)
            .bind(expires_at.to_rfc3339())
            .bind(commit_token.to_string())
            .execute(sq)
            .await?;
        }
    }

    cache.add_pending(developer_uuid, amount as i64, timeout_secs).await;

    // Store transaction data in cache
    use crate::models::DeductionTransaction;
    let tx = DeductionTransaction {
        id: 0,
        developer_uuid,
        transaction_token,
        amount,
        status: "pending".into(),
        created_at: now,
        expires_at,
        confirmed_at: None,
        commit_token: Some(commit_token.to_string()),
    };
    cache.set_transaction(&tx, timeout_secs).await;

    log::info!(
        "Deduction initiated: dev={}, amount={}, token={}, commit={}, available={}, limit={}",
        developer_uuid, amount, transaction_token, commit_token, available, limit
    );

    Ok(InitiateDeductionResponse { transaction_token, commit_token, expires_at })
}

pub async fn confirm_deduction(
    db: &DbPool,
    cache: &mut CacheBackend,
    req: ConfirmDeductionRequest,
) -> Result<(), AppError> {
    // Idempotency check
    if !cache.try_claim_processed(req.transaction_token).await {
        // Already processing — check current state
        let tx = match db {
            DbPool::Postgres(pg) => {
                sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT status FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(req.transaction_token)
                .fetch_optional(pg)
                .await?
            }
            DbPool::Sqlite(sq) => {
                sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT status FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(req.transaction_token.to_string())
                .fetch_optional(sq)
                .await?
            }
        };

        match tx {
            Some(t) if t.status == "pending" => {
                return Err(AppError::Conflict("Request already being processed".into()));
            }
            Some(t) => {
                return Err(AppError::Conflict(format!("Transaction already {}", t.status)));
            }
            None => {
                return Err(AppError::NotFound("Transaction not found".into()));
            }
        }
    }

    let result = confirm_deduction_impl(db, cache, &req).await;

    if result.is_ok() {
        cache.del_transaction(req.transaction_token).await;
    } else {
        cache.del_processed(req.transaction_token).await;
    }

    result
}

async fn confirm_deduction_impl(
    db: &DbPool,
    cache: &mut CacheBackend,
    req: &ConfirmDeductionRequest,
) -> Result<(), AppError> {
    let tx = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, DeductionTransaction>(
                "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
            )
            .bind(req.transaction_token)
            .fetch_optional(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, DeductionTransaction>(
                "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
            )
            .bind(req.transaction_token.to_string())
            .fetch_optional(sq)
            .await?
        }
    }
    .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

    if tx.status != "pending" {
        return Err(AppError::Conflict(format!(
            "Transaction already {}",
            tx.status
        )));
    }

    let commit_token = Uuid::parse_str(
        tx.commit_token.as_deref().ok_or_else(|| AppError::InternalError("Missing commit token".into()))?
    ).map_err(|_| AppError::InternalError("Invalid commit token format".into()))?;

    if commit_token != req.commit_token {
        return Err(AppError::BadRequest("Invalid commit token".into()));
    }

    if Utc::now() > tx.expires_at {
        match db {
            DbPool::Postgres(pg) => {
                sqlx::query("UPDATE deduction_transactions SET status = 'expired' WHERE id = $1")
                    .bind(tx.id)
                    .execute(pg)
                    .await?;
            }
            DbPool::Sqlite(sq) => {
                sqlx::query("UPDATE deduction_transactions SET status = 'expired' WHERE id = $1")
                    .bind(tx.id)
                    .execute(sq)
                    .await?;
            }
        }
        return Err(AppError::BadRequest("Transaction expired".into()));
    }

    let (deducted, new_available) = cache.deduct(tx.developer_uuid, tx.amount).await?;

    if deducted {
        sync_deduction_to_db(db, tx.developer_uuid, new_available).await?;
    } else {
        match db {
            DbPool::Postgres(pg) => {
                let mut tx_obj = pg.begin().await?;

                let result = sqlx::query(
                    r#"UPDATE developers SET
                       deduction_available = deduction_available - $1,
                       successful_auths = successful_auths + 1,
                       last_auth_time = NOW(),
                       updated_at = NOW()
                       WHERE developer_uuid = $2 AND deduction_available >= $1"#
                )
                .bind(tx.amount)
                .bind(tx.developer_uuid)
                .execute(&mut *tx_obj)
                .await?;

                if result.rows_affected() == 0 {
                    tx_obj.rollback().await?;
                    return Err(AppError::Conflict("Insufficient deduction available at confirm time".into()));
                }

                tx_obj.commit().await?;
            }
            DbPool::Sqlite(sq) => {
                let mut tx_obj = sq.begin().await?;

                let result = sqlx::query(
                    r#"UPDATE developers SET
                       deduction_available = deduction_available - $1,
                       successful_auths = successful_auths + 1,
                       last_auth_time = $2,
                       updated_at = $2
                       WHERE developer_uuid = $3 AND deduction_available >= $1"#
                )
                .bind(tx.amount)
                .bind(Utc::now().to_rfc3339())
                .bind(tx.developer_uuid.to_string())
                .execute(&mut *tx_obj)
                .await?;

                if result.rows_affected() == 0 {
                    tx_obj.rollback().await?;
                    return Err(AppError::Conflict("Insufficient deduction available at confirm time".into()));
                }

                tx_obj.commit().await?;
            }
        }

        let _ = get_or_sync_deduction(cache, db, tx.developer_uuid).await;
    }

    let now_str = Utc::now().to_rfc3339();
    match db {
        DbPool::Postgres(pg) => {
            sqlx::query("UPDATE deduction_transactions SET status = 'committed', confirmed_at = NOW() WHERE id = $1")
                .bind(tx.id)
                .execute(pg)
                .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query("UPDATE deduction_transactions SET status = 'committed', confirmed_at = $1 WHERE id = $2")
                .bind(now_str)
                .bind(tx.id)
                .execute(sq)
                .await?;
        }
    }

    cache.remove_pending(tx.developer_uuid, tx.amount as i64).await;

    log::info!(
        "Deduction confirmed: dev={}, amount={}, token={}",
        tx.developer_uuid, tx.amount, req.transaction_token
    );

    Ok(())
}

pub async fn cancel_deduction(
    db: &DbPool,
    cache: &mut CacheBackend,
    req: CancelDeductionRequest,
) -> Result<(), AppError> {
    // Idempotency check
    if !cache.try_claim_processed(req.transaction_token).await {
        return Err(AppError::Conflict("Request already being processed".into()));
    }

    let result = cancel_deduction_impl(db, cache, &req).await;

    if result.is_ok() {
        cache.del_transaction(req.transaction_token).await;
    } else {
        cache.del_processed(req.transaction_token).await;
    }

    result
}

async fn cancel_deduction_impl(
    db: &DbPool,
    cache: &mut CacheBackend,
    req: &CancelDeductionRequest,
) -> Result<(), AppError> {
    let tx = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, DeductionTransaction>(
                "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
            )
            .bind(req.transaction_token)
            .fetch_optional(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, DeductionTransaction>(
                "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
            )
            .bind(req.transaction_token.to_string())
            .fetch_optional(sq)
            .await?
        }
    }
    .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

    if tx.status != "pending" {
        return Err(AppError::Conflict(format!(
            "Transaction already {}",
            tx.status
        )));
    }

    match db {
        DbPool::Postgres(pg) => {
            sqlx::query(
                "UPDATE deduction_transactions SET status = 'cancelled' WHERE id = $1 AND status = 'pending'"
            )
            .bind(tx.id)
            .execute(pg)
            .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE deduction_transactions SET status = 'cancelled' WHERE id = $1 AND status = 'pending'"
            )
            .bind(tx.id)
            .execute(sq)
            .await?;
        }
    }

    cache.remove_pending(tx.developer_uuid, tx.amount as i64).await;

    log::info!(
        "Deduction cancelled: dev={}, amount={}, token={}",
        tx.developer_uuid, tx.amount, req.transaction_token
    );

    Ok(())
}

pub async fn expire_stale_transactions(db: &DbPool) -> Result<u64, AppError> {
    match db {
        DbPool::Postgres(pg) => {
            let result = sqlx::query(
                "UPDATE deduction_transactions SET status = 'expired' \
                 WHERE status = 'pending' AND expires_at < NOW()"
            )
            .execute(pg)
            .await?;
            Ok(result.rows_affected())
        }
        DbPool::Sqlite(sq) => {
            let result = sqlx::query(
                "UPDATE deduction_transactions SET status = 'expired' \
                 WHERE status = 'pending' AND expires_at < $1"
            )
            .bind(Utc::now().to_rfc3339())
            .execute(sq)
            .await?;
            Ok(result.rows_affected())
        }
    }
}

/// Sync cache pending counts with DB: count only non-expired pending transactions.
/// This fixes the window where expire_stale_transactions marks DB rows as expired
/// but the cache still counts them as pending.
pub async fn sync_pending_counts(db: &DbPool, cache: &mut CacheBackend) -> Result<(), AppError> {
    let dev_repo = crate::repositories::DeveloperRepository::new(db.clone());
    let all_devs = dev_repo.get_all().await?;

    for dev in &all_devs {
        let pending_amount = match db {
            DbPool::Postgres(pg) => {
                let row: (i64,) = sqlx::query_as(
                    "SELECT COALESCE(SUM(amount), 0) FROM deduction_transactions \
                     WHERE developer_uuid = $1 AND status = 'pending'"
                )
                .bind(dev.developer_uuid)
                .fetch_one(pg)
                .await?;
                row.0
            }
            DbPool::Sqlite(sq) => {
                let row: (i64,) = sqlx::query_as(
                    "SELECT COALESCE(SUM(amount), 0) FROM deduction_transactions \
                     WHERE developer_uuid = $1 AND status = 'pending'"
                )
                .bind(dev.developer_uuid.to_string())
                .fetch_one(sq)
                .await?;
                row.0
            }
        };

        if pending_amount > 0 {
            cache.add_pending(dev.developer_uuid, pending_amount, 7200).await;
        } else {
            // Ensure no stale pending count in cache
            cache.remove_pending(dev.developer_uuid, i64::MAX).await;
        }
    }

    Ok(())
}

/// Delete expired transactions that are older than the given duration.
/// This prevents the DB from accumulating unlimited expired rows.
pub async fn cleanup_expired_transactions(db: &DbPool, older_than_secs: i64) -> Result<u64, AppError> {
    let cutoff = Utc::now() - Duration::seconds(older_than_secs);
    match db {
        DbPool::Postgres(pg) => {
            let result = sqlx::query(
                "DELETE FROM deduction_transactions \
                 WHERE status = 'expired' AND expires_at < $1"
            )
            .bind(cutoff)
            .execute(pg)
            .await?;
            Ok(result.rows_affected())
        }
        DbPool::Sqlite(sq) => {
            let result = sqlx::query(
                "DELETE FROM deduction_transactions \
                 WHERE status = 'expired' AND expires_at < $1"
            )
            .bind(cutoff.to_rfc3339())
            .execute(sq)
            .await?;
            Ok(result.rows_affected())
        }
    }
}

pub async fn recover_deduction_for_all(
    db: &DbPool,
    cache: &mut CacheBackend,
) -> Result<u64, AppError> {
    const BATCH_SIZE: usize = 100;
    const BATCH_TIMEOUT_SECS: u64 = 30;

    let dev_repo = crate::repositories::DeveloperRepository::new(db.clone());
    let all_devs = dev_repo.get_all().await?;

    let mut recovered_count = 0u64;
    let now = Utc::now();

    for chunk in all_devs.chunks(BATCH_SIZE) {
        let batch_start = std::time::Instant::now();

        for dev in chunk {
            let last_recovery = dev.last_recovery_time.unwrap_or_else(|| now - Duration::days(1));
            let interval = Duration::seconds(dev.recovery_interval_secs as i64);

            if now - last_recovery < interval {
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
                    log::error!("Error updating deduction available: {}", e);
                }
                if let Err(e) = dev_repo.update_last_recovery_time(dev.developer_uuid).await {
                    log::error!("Error updating last recovery time: {}", e);
                }

                log::info!(
                    "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {}, interval: {}s)",
                    dev.developer_uuid, current_available, new_available, limit, recovery_amount, dev.recovery_interval_secs
                );
                recovered_count += 1;
            }
        }

        if batch_start.elapsed().as_secs() >= BATCH_TIMEOUT_SECS {
            log::warn!("Recovery batch timed out after {}s, processed {}/{} developers",
                BATCH_TIMEOUT_SECS, recovered_count, all_devs.len());
            break;
        }
    }

    Ok(recovered_count)
}

/// Rebuild cache state from DB on startup.
/// This ensures pending counts are correct after a restart,
/// preventing over-deduction when memory cache is empty but DB has pending transactions.
pub async fn rebuild_cache_state(db: &DbPool, cache: &mut CacheBackend) -> Result<(), AppError> {
    let dev_repo = crate::repositories::DeveloperRepository::new(db.clone());
    let all_devs = dev_repo.get_all().await?;

    let mut pending_map: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();

    // Scan all pending transactions in DB
    for dev in &all_devs {
        match db {
            DbPool::Postgres(pg) => {
                let txs: Vec<(i64, i32)> = sqlx::query_as(
                    "SELECT id, amount FROM deduction_transactions WHERE developer_uuid = $1 AND status = 'pending'"
                )
                .bind(dev.developer_uuid)
                .fetch_all(pg)
                .await?;
                let total_pending: i64 = txs.iter().map(|(_, amount)| *amount as i64).sum();
                pending_map.insert(dev.developer_uuid, total_pending);
            }
            DbPool::Sqlite(sq) => {
                let txs: Vec<(i64, i32)> = sqlx::query_as(
                    "SELECT id, amount FROM deduction_transactions WHERE developer_uuid = $1 AND status = 'pending'"
                )
                .bind(dev.developer_uuid.to_string())
                .fetch_all(sq)
                .await?;
                let total_pending: i64 = txs.iter().map(|(_, amount)| *amount as i64).sum();
                pending_map.insert(dev.developer_uuid, total_pending);
            }
        }
    }

    // Rebuild cache
    for dev in &all_devs {
        let pending = pending_map.get(&dev.developer_uuid).copied().unwrap_or(0);
        cache.set_deduction_data(dev.developer_uuid, dev.deduction_available, dev.deduction_limit).await;
        if pending > 0 {
            cache.add_pending(dev.developer_uuid, pending, 7200).await;
        }
    }

    let total_pending_devs = pending_map.values().filter(|&&p| p > 0).count();
    log::info!("[Startup] Cache state rebuilt: {} developers loaded, {} with pending transactions",
        all_devs.len(), total_pending_devs);

    Ok(())
}
