use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use crate::db::DbPool;
use crate::models::{DeductionTransaction, ConfirmDeductionRequest};
use crate::errors::AppError;

#[allow(dead_code)]
pub struct TransactionRepository {
    pool: DbPool,
}

impl TransactionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        developer_uuid: Uuid,
        amount: i32,
        timeout_secs: u64,
    ) -> Result<(Uuid, Uuid, DateTime<Utc>), AppError> {
        let transaction_token = Uuid::new_v4();
        let commit_token = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = now + Duration::seconds(timeout_secs as i64);

        match &self.pool {
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

        Ok((transaction_token, commit_token, expires_at))
    }

    pub async fn get_by_token(&self, tx_token: Uuid) -> Result<Option<DeductionTransaction>, AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                Ok(sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(tx_token)
                .fetch_optional(pg)
                .await?)
            }
            DbPool::Sqlite(sq) => {
                Ok(sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(tx_token.to_string())
                .fetch_optional(sq)
                .await?)
            }
        }
    }

    pub async fn confirm(&self, tx_token: Uuid) -> Result<(), AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                sqlx::query(
                    "UPDATE deduction_transactions SET status = 'committed', confirmed_at = NOW() \
                     WHERE transaction_token = $1"
                )
                .bind(tx_token)
                .execute(pg)
                .await?;
            }
            DbPool::Sqlite(sq) => {
                sqlx::query(
                    "UPDATE deduction_transactions SET status = 'committed', confirmed_at = $1 \
                     WHERE transaction_token = $2"
                )
                .bind(Utc::now().to_rfc3339())
                .bind(tx_token.to_string())
                .execute(sq)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn cancel(&self, tx_token: Uuid) -> Result<bool, AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                let result = sqlx::query(
                    "UPDATE deduction_transactions SET status = 'cancelled' \
                     WHERE transaction_token = $1 AND status = 'pending'"
                )
                .bind(tx_token)
                .execute(pg)
                .await?;
                Ok(result.rows_affected() > 0)
            }
            DbPool::Sqlite(sq) => {
                let result = sqlx::query(
                    "UPDATE deduction_transactions SET status = 'cancelled' \
                     WHERE transaction_token = $1 AND status = 'pending'"
                )
                .bind(tx_token.to_string())
                .execute(sq)
                .await?;
                Ok(result.rows_affected() > 0)
            }
        }
    }

    pub async fn expire_stale(&self) -> Result<u64, AppError> {
        match &self.pool {
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

    pub async fn list_paginated(
        &self,
        page: i64,
        page_size: i64,
        dev_uuid: Option<Uuid>,
        status: Option<&str>,
    ) -> Result<(Vec<DeductionTransaction>, i64), AppError> {
        let offset = (page - 1) * page_size;

        let (count_query, data_query) = if dev_uuid.is_some() {
            if status.is_some() {
                (
                    "SELECT COUNT(*) FROM deduction_transactions WHERE developer_uuid = $1 AND status = $2",
                    "SELECT * FROM deduction_transactions WHERE developer_uuid = $1 AND status = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4"
                )
            } else {
                (
                    "SELECT COUNT(*) FROM deduction_transactions WHERE developer_uuid = $1",
                    "SELECT * FROM deduction_transactions WHERE developer_uuid = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                )
            }
        } else {
            if status.is_some() {
                (
                    "SELECT COUNT(*) FROM deduction_transactions WHERE status = $1",
                    "SELECT * FROM deduction_transactions WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3"
                )
            } else {
                (
                    "SELECT COUNT(*) FROM deduction_transactions",
                    "SELECT * FROM deduction_transactions ORDER BY created_at DESC LIMIT $1 OFFSET $2"
                )
            }
        };

        let (count, txs) = match &self.pool {
            DbPool::Postgres(pg) => {
                let count: (i64,) = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
                    sqlx::query_as(count_query).bind(uuid).bind(s).fetch_one(pg).await?
                } else if let Some(uuid) = dev_uuid {
                    sqlx::query_as(count_query).bind(uuid).fetch_one(pg).await?
                } else if let Some(s) = status {
                    sqlx::query_as(count_query).bind(s).fetch_one(pg).await?
                } else {
                    sqlx::query_as(count_query).fetch_one(pg).await?
                };

                let txs: Vec<DeductionTransaction> = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
                    sqlx::query_as(data_query).bind(uuid).bind(s).bind(page_size).bind(offset).fetch_all(pg).await?
                } else if let Some(uuid) = dev_uuid {
                    sqlx::query_as(data_query).bind(uuid).bind(page_size).bind(offset).fetch_all(pg).await?
                } else if let Some(s) = status {
                    sqlx::query_as(data_query).bind(s).bind(page_size).bind(offset).fetch_all(pg).await?
                } else {
                    sqlx::query_as(data_query).bind(page_size).bind(offset).fetch_all(pg).await?
                };

                (count.0, txs)
            }
            DbPool::Sqlite(sq) => {
                let count: (i64,) = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
                    sqlx::query_as(count_query).bind(uuid.to_string()).bind(s).fetch_one(sq).await?
                } else if let Some(uuid) = dev_uuid {
                    sqlx::query_as(count_query).bind(uuid.to_string()).fetch_one(sq).await?
                } else if let Some(s) = status {
                    sqlx::query_as(count_query).bind(s).fetch_one(sq).await?
                } else {
                    sqlx::query_as(count_query).fetch_one(sq).await?
                };

                let txs: Vec<DeductionTransaction> = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
                    sqlx::query_as(data_query).bind(uuid.to_string()).bind(s).bind(page_size).bind(offset).fetch_all(sq).await?
                } else if let Some(uuid) = dev_uuid {
                    sqlx::query_as(data_query).bind(uuid.to_string()).bind(page_size).bind(offset).fetch_all(sq).await?
                } else if let Some(s) = status {
                    sqlx::query_as(data_query).bind(s).bind(page_size).bind(offset).fetch_all(sq).await?
                } else {
                    sqlx::query_as(data_query).bind(page_size).bind(offset).fetch_all(sq).await?
                };

                (count.0, txs)
            }
        };

        Ok((txs, count))
    }

    pub async fn validate_for_confirm(
        &self,
        req: &ConfirmDeductionRequest,
    ) -> Result<DeductionTransaction, AppError> {
        let tx = match &self.pool {
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
            return Err(AppError::Conflict(format!("Transaction already {}", tx.status)));
        }

        let commit_token = Uuid::parse_str(
            tx.commit_token.as_deref().ok_or_else(|| AppError::InternalError("Missing commit token".into()))?
        ).map_err(|_| AppError::InternalError("Invalid commit token format".into()))?;

        if commit_token != req.commit_token {
            return Err(AppError::BadRequest("Invalid commit token".into()));
        }

        if Utc::now() > tx.expires_at {
            match &self.pool {
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

        Ok(tx)
    }

    pub async fn validate_for_cancel(&self, tx_token: Uuid) -> Result<DeductionTransaction, AppError> {
        let tx = match &self.pool {
            DbPool::Postgres(pg) => {
                sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(tx_token)
                .fetch_optional(pg)
                .await?
            }
            DbPool::Sqlite(sq) => {
                sqlx::query_as::<_, DeductionTransaction>(
                    "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
                )
                .bind(tx_token.to_string())
                .fetch_optional(sq)
                .await?
            }
        }
        .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

        if tx.status != "pending" {
            return Err(AppError::Conflict(format!("Transaction already {}", tx.status)));
        }

        Ok(tx)
    }
}
