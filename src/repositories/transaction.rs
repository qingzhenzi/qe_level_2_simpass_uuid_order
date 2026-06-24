use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use crate::models::{DeductionTransaction, ConfirmDeductionRequest};
use crate::errors::AppError;

#[allow(dead_code)]
pub struct TransactionRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    pub async fn create(
        &self,
        developer_uuid: Uuid,
        amount: i32,
        timeout_secs: u64,
    ) -> Result<(Uuid, Uuid, DateTime<Utc>), AppError> {
        let transaction_token = Uuid::new_v4();
        let commit_token = Uuid::new_v4();
        let expires_at = Utc::now() + Duration::seconds(timeout_secs as i64);

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
        .execute(self.pool)
        .await?;

        Ok((transaction_token, commit_token, expires_at))
    }

    pub async fn get_by_token(&self, tx_token: Uuid) -> Result<Option<DeductionTransaction>, AppError> {
        let tx = sqlx::query_as::<_, DeductionTransaction>(
            "SELECT * FROM deduction_transactions WHERE transaction_token = $1"
        )
        .bind(tx_token)
        .fetch_optional(self.pool)
        .await?;
        Ok(tx)
    }

    #[allow(dead_code)]
    pub async fn confirm(&self, tx_token: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE deduction_transactions SET status = 'committed', confirmed_at = NOW() 
             WHERE transaction_token = $1"
        )
        .bind(tx_token)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn cancel(&self, tx_token: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE deduction_transactions SET status = 'cancelled' 
             WHERE transaction_token = $1 AND status = 'pending'"
        )
        .bind(tx_token)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    #[allow(dead_code)]
    pub async fn expire_stale(&self) -> Result<u64, AppError> {
        let result = sqlx::query(
            "UPDATE deduction_transactions SET status = 'expired' 
             WHERE status = 'pending' AND expires_at < NOW()"
        )
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_paginated(
        &self,
        page: i64,
        page_size: i64,
        dev_uuid: Option<Uuid>,
        status: Option<&str>,
    ) -> Result<(Vec<DeductionTransaction>, i64), AppError> {
        let offset = (page - 1) * page_size;

        let (count_query, data_query) = if let Some(_uuid) = dev_uuid {
            if let Some(_s) = status {
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
            if let Some(_s) = status {
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

        let count: (i64,) = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
            sqlx::query_as(count_query).bind(uuid).bind(s).fetch_one(self.pool).await?
        } else if let Some(uuid) = dev_uuid {
            sqlx::query_as(count_query).bind(uuid).fetch_one(self.pool).await?
        } else if let Some(s) = status {
            sqlx::query_as(count_query).bind(s).fetch_one(self.pool).await?
        } else {
            sqlx::query_as(count_query).fetch_one(self.pool).await?
        };

        let txs: Vec<DeductionTransaction> = if let (Some(uuid), Some(s)) = (dev_uuid, status) {
            sqlx::query_as(data_query).bind(uuid).bind(s).bind(page_size).bind(offset).fetch_all(self.pool).await?
        } else if let Some(uuid) = dev_uuid {
            sqlx::query_as(data_query).bind(uuid).bind(page_size).bind(offset).fetch_all(self.pool).await?
        } else if let Some(s) = status {
            sqlx::query_as(data_query).bind(s).bind(page_size).bind(offset).fetch_all(self.pool).await?
        } else {
            sqlx::query_as(data_query).bind(page_size).bind(offset).fetch_all(self.pool).await?
        };

        Ok((txs, count.0))
    }

    #[allow(dead_code)]
    pub async fn validate_for_confirm(
        &self,
        req: &ConfirmDeductionRequest,
    ) -> Result<DeductionTransaction, AppError> {
        let tx = self.get_by_token(req.transaction_token).await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

        if tx.status != "pending" {
            return Err(AppError::Conflict(format!("Transaction already {}", tx.status)));
        }

        let commit_token = tx.commit_token
            .ok_or_else(|| AppError::InternalError("Missing commit token".into()))?;

        if commit_token != req.commit_token {
            return Err(AppError::BadRequest("Invalid commit token".into()));
        }

        if Utc::now() > tx.expires_at {
            sqlx::query("UPDATE deduction_transactions SET status = 'expired' WHERE id = $1")
                .bind(tx.id)
                .execute(self.pool)
                .await?;
            return Err(AppError::BadRequest("Transaction expired".into()));
        }

        Ok(tx)
    }

    #[allow(dead_code)]
    pub async fn validate_for_cancel(&self, tx_token: Uuid) -> Result<DeductionTransaction, AppError> {
        let tx = self.get_by_token(tx_token).await?
            .ok_or_else(|| AppError::NotFound("Transaction not found".into()))?;

        if tx.status != "pending" {
            return Err(AppError::Conflict(format!("Transaction already {}", tx.status)));
        }

        Ok(tx)
    }
}
