use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use crate::models::Developer;
use crate::errors::AppError;

pub struct DeveloperRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> DeveloperRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_uuid(&self, dev_uuid: Uuid) -> Result<Option<Developer>, AppError> {
        let dev = sqlx::query_as::<_, Developer>(
            "SELECT * FROM developers WHERE developer_uuid = $1"
        )
        .bind(dev_uuid)
        .fetch_optional(self.pool)
        .await?;
        Ok(dev)
    }

    pub async fn get_all(&self) -> Result<Vec<Developer>, AppError> {
        let devs = sqlx::query_as::<_, Developer>("SELECT * FROM developers")
            .fetch_all(self.pool)
            .await?;
        Ok(devs)
    }

    pub async fn update_deduction_available(&self, dev_uuid: Uuid, new_available: i32) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE developers SET deduction_available = $1, updated_at = NOW() WHERE developer_uuid = $2"
        )
        .bind(new_available)
        .bind(dev_uuid)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_last_recovery_time(&self, dev_uuid: Uuid) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE developers SET last_recovery_time = NOW() WHERE developer_uuid = $1"
        )
        .bind(dev_uuid)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn deduct_with_transaction(
        &self,
        dev_uuid: Uuid,
        amount: i32,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await?;

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
}
