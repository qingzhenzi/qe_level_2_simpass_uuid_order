//! Deduction recovery task.
//!
//! This module is kept for backward compatibility.
//! The actual recovery logic now lives in `tasks/expiration.rs`.

use sqlx::PgPool;
use redis::aio::ConnectionManager;
use uuid::Uuid;
use crate::errors::AppError;

/// Recover deduction for a single developer.
/// Delegates to `tasks::expiration::recover_deduction_for_developer`.
#[allow(dead_code)]
pub async fn recover_deduction_for_developer(
    pool: &PgPool,
    redis_conn: &mut Option<ConnectionManager>,
    redis_prefix: &str,
    dev_uuid: Uuid,
    _recovery_amount: i32,
) -> Result<i32, AppError> {
    crate::tasks::expiration::recover_deduction_for_developer(
        pool, redis_conn, redis_prefix, dev_uuid,
    ).await
}
