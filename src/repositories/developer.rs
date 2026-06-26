use uuid::Uuid;
use crate::db::DbPool;
use crate::models::Developer;
use crate::errors::AppError;

#[allow(dead_code)]
pub struct DeveloperRepository {
    pool: DbPool,
}

impl DeveloperRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_uuid(&self, dev_uuid: Uuid) -> Result<Option<Developer>, AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                Ok(sqlx::query_as::<_, Developer>(
                    "SELECT * FROM developers WHERE developer_uuid = $1"
                )
                .bind(dev_uuid)
                .fetch_optional(pg)
                .await?)
            }
            DbPool::Sqlite(sq) => {
                Ok(sqlx::query_as::<_, Developer>(
                    "SELECT * FROM developers WHERE developer_uuid = $1"
                )
                .bind(dev_uuid.to_string())
                .fetch_optional(sq)
                .await?)
            }
        }
    }

    pub async fn get_all(&self) -> Result<Vec<Developer>, AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                Ok(sqlx::query_as::<_, Developer>("SELECT * FROM developers")
                    .fetch_all(pg)
                    .await?)
            }
            DbPool::Sqlite(sq) => {
                Ok(sqlx::query_as::<_, Developer>("SELECT * FROM developers")
                    .fetch_all(sq)
                    .await?)
            }
        }
    }

    pub async fn update_deduction_available(&self, dev_uuid: Uuid, new_available: i32) -> Result<(), AppError> {
        match &self.pool {
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
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(dev_uuid.to_string())
                .execute(sq)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn update_last_recovery_time(&self, dev_uuid: Uuid) -> Result<(), AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                sqlx::query(
                    "UPDATE developers SET last_recovery_time = NOW() WHERE developer_uuid = $1"
                )
                .bind(dev_uuid)
                .execute(pg)
                .await?;
            }
            DbPool::Sqlite(sq) => {
                sqlx::query(
                    "UPDATE developers SET last_recovery_time = $1 WHERE developer_uuid = $2"
                )
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(dev_uuid.to_string())
                .execute(sq)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn deduct_with_transaction(
        &self,
        dev_uuid: Uuid,
        amount: i32,
    ) -> Result<bool, AppError> {
        match &self.pool {
            DbPool::Postgres(pg) => {
                let mut tx = pg.begin().await?;

                let result = sqlx::query(
                    r#"UPDATE developers SET
                       deduction_available = deduction_available - $1,
                       successful_auths = successful_auths + 1,
                       last_auth_time = NOW(),
                       updated_at = NOW()
                       WHERE developer_uuid = $2 AND deduction_available >= $1"#
                )
                .bind(amount)
                .bind(dev_uuid)
                .execute(&mut *tx)
                .await?;

                if result.rows_affected() == 0 {
                    tx.rollback().await?;
                    Ok(false)
                } else {
                    tx.commit().await?;
                    Ok(true)
                }
            }
            DbPool::Sqlite(sq) => {
                let mut tx = sq.begin().await?;

                let result = sqlx::query(
                    r#"UPDATE developers SET
                       deduction_available = deduction_available - $1,
                       successful_auths = successful_auths + 1,
                       last_auth_time = $2,
                       updated_at = $2
                       WHERE developer_uuid = $3 AND deduction_available >= $1"#
                )
                .bind(amount)
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(dev_uuid.to_string())
                .execute(&mut *tx)
                .await?;

                if result.rows_affected() == 0 {
                    tx.rollback().await?;
                    Ok(false)
                } else {
                    tx.commit().await?;
                    Ok(true)
                }
            }
        }
    }
}
