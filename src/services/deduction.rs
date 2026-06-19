use chrono::{Duration, Utc};
use uuid::Uuid;
use sqlx::PgPool;
use redis::aio::ConnectionManager;

use crate::errors::AppError;
use crate::models::{
    DeductionTransaction, InitiateDeductionResponse,
    ConfirmDeductionRequest, CancelDeductionRequest, Developer,
};

const DEDUCTION_AVAILABLE_FIELD: &str = "deduction_available";
const DEDUCTION_LIMIT_FIELD: &str = "deduction_limit";

fn redis_dev_key(redis_prefix: &str, dev_uuid: Uuid) -> String {
    format!("{}:dev:{}", redis_prefix, dev_uuid)
}

fn redis_pending_key(redis_prefix: &str, dev_uuid: Uuid) -> String {
    format!("{}:dev:{}:pending", redis_prefix, dev_uuid)
}

pub async fn get_or_sync_deduction_from_redis(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
) -> Result<(i32, i32), AppError> {
    let redis_key = redis_dev_key(redis_prefix, dev_uuid);

    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let available: Option<i32> = conn.hget(&redis_key, DEDUCTION_AVAILABLE_FIELD).await.ok().flatten();
        let limit: Option<i32> = conn.hget(&redis_key, DEDUCTION_LIMIT_FIELD).await.ok().flatten();

        if let (Some(avail), Some(lim)) = (available, limit) {
            return Ok((avail, lim));
        }
    }

    let dev = sqlx::query_as::<_, Developer>(
        "SELECT * FROM developers WHERE developer_uuid = $1"
    )
    .bind(dev_uuid)
    .fetch_optional(pg_pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Developer not found".into()))?;

    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let _: Result<(), _> = conn.hset_multiple(&redis_key, &[
            (DEDUCTION_AVAILABLE_FIELD, dev.deduction_available as i64),
            (DEDUCTION_LIMIT_FIELD, dev.deduction_limit as i64),
        ]).await;
        log::info!("Synced deduction data to Redis for dev: {}", dev_uuid);
    }

    Ok((dev.deduction_available, dev.deduction_limit))
}

pub async fn get_pending_amount_from_redis(
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
) -> i32 {
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let pending_key = redis_pending_key(redis_prefix, dev_uuid);
        let pending: i64 = conn.get(&pending_key).await.unwrap_or(0);
        return pending as i32;
    }
    0
}

pub async fn add_pending_to_redis(
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
    amount: i32,
    timeout_secs: u64,
) {
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let pending_key = redis_pending_key(redis_prefix, dev_uuid);
        let _: Result<i64, _> = conn.incr(&pending_key, amount).await;
        let _: Result<(), _> = conn.expire(&pending_key, timeout_secs as i64).await;
    }
}

pub async fn remove_pending_from_redis(
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
    amount: i32,
) {
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let pending_key = redis_pending_key(redis_prefix, dev_uuid);
        let _: Result<i64, _> = conn.decr(&pending_key, amount).await;
    }
}

pub async fn deduct_from_redis(
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
    amount: i32,
) -> Result<(bool, i32), AppError> {
    if let Some(ref mut conn) = redis_conn {
        let redis_key = redis_dev_key(redis_prefix, dev_uuid);

        let script = r#"
            local key = KEYS[1]
            local amount = tonumber(ARGV[1])
            local current = tonumber(redis.call('HGET', key, 'deduction_available'))
            if current == nil then
                return {-1, 0}
            end
            if current < amount then
                return {0, current}
            end
            local new_val = current - amount
            redis.call('HSET', key, 'deduction_available', new_val)
            return {1, new_val}
        "#;

        let result: (i64, i64) = redis::cmd("EVAL")
            .arg(script)
            .arg(1)
            .arg(&redis_key)
            .arg(amount)
            .query_async(conn)
            .await
            .map_err(|e| AppError::RedisError(e.to_string()))?;

        match result.0 {
            1 => {
                log::info!("Deducted {} from Redis for dev {}, new value: {}", amount, dev_uuid, result.1);
                return Ok((true, result.1 as i32));
            }
            0 => {
                return Ok((false, result.1 as i32));
            }
            -1 => {
                log::warn!("Deduction data not in Redis for dev {}, need sync", dev_uuid);
                return Ok((false, 0));
            }
            _ => return Ok((false, 0)),
        }
    }
    Ok((false, 0))
}

pub async fn sync_deduction_to_pg(
    pg_pool: &PgPool,
    dev_uuid: Uuid,
    new_available: i32,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE developers SET deduction_available = $1, updated_at = NOW() WHERE developer_uuid = $2"
    )
    .bind(new_available)
    .bind(dev_uuid)
    .execute(pg_pool)
    .await?;

    log::info!("Synced deduction_available={} to PostgreSQL for dev {}", new_available, dev_uuid);
    Ok(())
}

pub async fn initiate_deduction(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    developer_uuid: Uuid,
    amount: i32,
    timeout_secs: u64,
) -> Result<InitiateDeductionResponse, AppError> {
    let (available, limit) = get_or_sync_deduction_from_redis(
        pg_pool,
        redis_conn,
        redis_prefix,
        developer_uuid,
    ).await?;

    let pending_amount = get_pending_amount_from_redis(
        redis_conn,
        redis_prefix,
        developer_uuid,
    ).await;

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
    .execute(pg_pool)
    .await?;

    add_pending_to_redis(
        redis_conn,
        redis_prefix,
        developer_uuid,
        amount,
        timeout_secs,
    ).await;

    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let redis_key = format!("{}:deduct:{}", redis_prefix, transaction_token);
        let redis_data = serde_json::json!({
            "developer_uuid": developer_uuid.to_string(),
            "amount": amount,
            "commit_token": commit_token.to_string(),
            "status": "pending",
            "expires_at": expires_at.to_rfc3339(),
        });
        let _: Result<(), _> = conn.set_ex(
            redis_key.as_str(),
            redis_data.to_string(),
            timeout_secs as u64,
        ).await;
    }

    log::info!(
        "Deduction initiated: dev={}, amount={}, token={}, commit={}, available={}, limit={}",
        developer_uuid, amount, transaction_token, commit_token, available, limit
    );

    Ok(InitiateDeductionResponse { transaction_token, commit_token, expires_at })
}

pub async fn confirm_deduction(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    req: ConfirmDeductionRequest,
) -> Result<(), AppError> {
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let idem_key = format!("{}:processed:{}", redis_prefix, req.transaction_token);
        match conn.set_nx(&idem_key, "confirmed").await {
            Ok(true) => {
                // Successfully claimed — proceed
                let _: Result<(), _> = conn.expire(&idem_key, 60).await;
            }
            Ok(false) => {
                // Key already exists — check PG for actual status
                let tx = sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT status FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(req.transaction_token)
                .fetch_optional(pg_pool)
                .await?;

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
            Err(e) => {
                // Redis error — skip idempotency check, let PG handle it
                log::warn!("Redis error in confirm_deduction idempotency check (skip): {}", e);
            }
        }
    }

    let result = confirm_deduction_impl(pg_pool, redis_conn, redis_prefix, &req).await;

    if result.is_ok() {
        if let Some(ref mut conn) = redis_conn {
            use redis::AsyncCommands;
            let redis_key = format!("{}:deduct:{}", redis_prefix, req.transaction_token);
            let _: Result<(), _> = conn.del(&redis_key).await;
        }
    } else if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let idem_key = format!("{}:processed:{}", redis_prefix, req.transaction_token);
        let _: Result<(), _> = conn.del(&idem_key).await;
    }

    result
}

async fn confirm_deduction_impl(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    req: &ConfirmDeductionRequest,
) -> Result<(), AppError> {
    let tx = sqlx::query_as::<_, DeductionTransaction>(
        "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
    )
    .bind(req.transaction_token)
    .fetch_optional(pg_pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

    if tx.status != "pending" {
        return Err(AppError::Conflict(format!(
            "Transaction already {}",
            tx.status
        )));
    }

    let commit_token = tx.commit_token
        .ok_or_else(|| AppError::InternalError("Missing commit token".into()))?;

    if commit_token != req.commit_token {
        return Err(AppError::BadRequest("Invalid commit token".into()));
    }

    if Utc::now() > tx.expires_at {
        sqlx::query("UPDATE deduction_transactions SET status = 'expired' WHERE id = $1")
            .bind(tx.id)
            .execute(pg_pool)
            .await?;
        return Err(AppError::BadRequest("Transaction expired".into()));
    }

    let (deducted, new_available) = deduct_from_redis(redis_conn, redis_prefix, tx.developer_uuid, tx.amount).await?;

    if deducted {
        sync_deduction_to_pg(pg_pool, tx.developer_uuid, new_available).await?;
    } else {
        let mut pg_tx = pg_pool.begin().await?;

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
        .execute(&mut *pg_tx)
        .await?;

        if result.rows_affected() == 0 {
            pg_tx.rollback().await?;
            return Err(AppError::Conflict("Insufficient deduction available at confirm time".into()));
        }

        pg_tx.commit().await?;

        let _ = get_or_sync_deduction_from_redis(
            pg_pool,
            redis_conn,
            redis_prefix,
            tx.developer_uuid,
        ).await;
    }

    sqlx::query("UPDATE deduction_transactions SET status = 'committed', confirmed_at = NOW() WHERE id = $1")
        .bind(tx.id)
        .execute(pg_pool)
        .await?;

    remove_pending_from_redis(redis_conn, redis_prefix, tx.developer_uuid, tx.amount).await;

    log::info!(
        "Deduction confirmed: dev={}, amount={}, token={}",
        tx.developer_uuid, tx.amount, req.transaction_token
    );

    Ok(())
}

pub async fn cancel_deduction(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    req: CancelDeductionRequest,
) -> Result<(), AppError> {
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let idem_key = format!("{}:processed:{}", redis_prefix, req.transaction_token);
        match conn.set_nx(&idem_key, "cancelled").await {
            Ok(true) => {
                let _: Result<(), _> = conn.expire(&idem_key, 60).await;
            }
            Ok(false) => {
                return Err(AppError::Conflict("Request already being processed".into()));
            }
            Err(e) => {
                log::warn!("Redis error in cancel_deduction idempotency check (skip): {}", e);
            }
        }
    }

    let result = cancel_deduction_impl(pg_pool, redis_conn, redis_prefix, &req).await;

    if result.is_ok() {
        if let Some(ref mut conn) = redis_conn {
            use redis::AsyncCommands;
            let redis_key = format!("{}:deduct:{}", redis_prefix, req.transaction_token);
            let _: Result<(), _> = conn.del(&redis_key).await;
        }
    } else if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let idem_key = format!("{}:processed:{}", redis_prefix, req.transaction_token);
        let _: Result<(), _> = conn.del(&idem_key).await;
    }

    result
}

async fn cancel_deduction_impl(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    req: &CancelDeductionRequest,
) -> Result<(), AppError> {
    let tx = sqlx::query_as::<_, DeductionTransaction>(
        "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
    )
    .bind(req.transaction_token)
    .fetch_optional(pg_pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

    if tx.status != "pending" {
        return Err(AppError::Conflict(format!(
            "Transaction already {}",
            tx.status
        )));
    }

    sqlx::query(
        "UPDATE deduction_transactions SET status = 'cancelled' WHERE id = $1 AND status = 'pending'"
    )
    .bind(tx.id)
    .execute(pg_pool)
    .await?;

    remove_pending_from_redis(redis_conn, redis_prefix, tx.developer_uuid, tx.amount).await;

    log::info!(
        "Deduction cancelled: dev={}, amount={}, token={}",
        tx.developer_uuid, tx.amount, req.transaction_token
    );

    Ok(())
}

pub async fn expire_stale_transactions(pg_pool: &PgPool) -> Result<u64, AppError> {
    let result = sqlx::query(
        "UPDATE deduction_transactions SET status = 'expired' 
         WHERE status = 'pending' AND expires_at < NOW()"
    )
    .execute(pg_pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn recover_deduction_for_all(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
) -> Result<u64, AppError> {
    let now = Utc::now();
    
    let devs = sqlx::query_as::<_, Developer>(
        "SELECT * FROM developers"
    )
    .fetch_all(pg_pool)
    .await?;

    let mut recovered_count = 0u64;

    for dev in devs {
        let last_recovery = dev.last_recovery_time.unwrap_or_else(|| Utc::now() - Duration::days(1));
        let interval = Duration::seconds(dev.recovery_interval_secs as i64);
        
        if now - last_recovery < interval {
            continue;
        }

        let redis_key = redis_dev_key(redis_prefix, dev.developer_uuid);

        let (current_available, limit) = if let Some(ref mut conn) = redis_conn {
            use redis::AsyncCommands;
            let avail: Option<i32> = conn.hget(&redis_key, DEDUCTION_AVAILABLE_FIELD).await.ok().flatten();
            let lim: Option<i32> = conn.hget(&redis_key, DEDUCTION_LIMIT_FIELD).await.ok().flatten();

            if let (Some(a), Some(l)) = (avail, lim) {
                (a, l)
            } else {
                (dev.deduction_available, dev.deduction_limit)
            }
        } else {
            (dev.deduction_available, dev.deduction_limit)
        };

        if current_available >= limit {
            continue;
        }

        let recovery_amount = dev.recovery_amount;
        let new_available = std::cmp::min(current_available + recovery_amount, limit);

        if new_available != current_available {
            if let Some(ref mut conn) = redis_conn {
                use redis::AsyncCommands;
                let _: Result<(), _> = conn.hset(&redis_key, DEDUCTION_AVAILABLE_FIELD, new_available).await;
            }

            sync_deduction_to_pg(pg_pool, dev.developer_uuid, new_available).await?;

            sqlx::query(
                "UPDATE developers SET last_recovery_time = NOW() WHERE developer_uuid = $1"
            )
            .bind(dev.developer_uuid)
            .execute(pg_pool)
            .await?;

            log::info!(
                "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {}, interval: {}s)",
                dev.developer_uuid, current_available, new_available, limit, recovery_amount, dev.recovery_interval_secs
            );
            recovered_count += 1;
        }
    }

    Ok(recovered_count)
}

pub async fn recover_deduction_for_developer(
    pg_pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
    recovery_amount: i32,
) -> Result<i32, AppError> {
    let (current_available, limit) = get_or_sync_deduction_from_redis(
        pg_pool,
        redis_conn,
        redis_prefix,
        dev_uuid,
    ).await?;

    if current_available >= limit {
        return Ok(current_available);
    }

    let new_available = std::cmp::min(current_available + recovery_amount, limit);

    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let redis_key = redis_dev_key(redis_prefix, dev_uuid);
        let _: Result<(), _> = conn.hset(&redis_key, DEDUCTION_AVAILABLE_FIELD, new_available).await;
    }

    sync_deduction_to_pg(pg_pool, dev_uuid, new_available).await?;

    log::info!(
        "Recovered deduction for dev {}: {} -> {} (limit: {}, recovery: {})",
        dev_uuid, current_available, new_available, limit, recovery_amount
    );

    Ok(new_available)
}
