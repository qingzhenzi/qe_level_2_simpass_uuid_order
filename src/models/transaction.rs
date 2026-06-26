use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, Error};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeductionTransaction {
    pub id: i64,
    pub developer_uuid: Uuid,
    pub transaction_token: Uuid,
    pub amount: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub commit_token: Option<String>,
}

impl<'r> FromRow<'r, PgRow> for DeductionTransaction {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(DeductionTransaction {
            id: row.try_get("id")?,
            developer_uuid: row.try_get("developer_uuid")?,
            transaction_token: row.try_get("transaction_token")?,
            amount: row.try_get("amount")?,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
            confirmed_at: row.try_get("confirmed_at")?,
            commit_token: row.try_get("commit_token")?,
        })
    }
}

impl<'r> FromRow<'r, SqliteRow> for DeductionTransaction {
    fn from_row(row: &SqliteRow) -> Result<Self, Error> {
        let dev_uuid_str: String = row.try_get("developer_uuid")?;
        let developer_uuid = Uuid::parse_str(&dev_uuid_str)
            .map_err(|e| Error::ColumnDecode {
                index: "developer_uuid".into(),
                source: Box::new(e),
            })?;

        let tx_token_str: String = row.try_get("transaction_token")?;
        let transaction_token = Uuid::parse_str(&tx_token_str)
            .map_err(|e| Error::ColumnDecode {
                index: "transaction_token".into(),
                source: Box::new(e),
            })?;

        let commit_token: Option<String> = row.try_get("commit_token")?;

        Ok(DeductionTransaction {
            id: row.try_get("id")?,
            developer_uuid,
            transaction_token,
            amount: row.try_get("amount")?,
            status: row.try_get("status")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
            confirmed_at: row.try_get("confirmed_at")?,
            commit_token,
        })
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct InitiateDeductionRequest {
    pub developer_uuid: Uuid,
    pub amount: i32,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct InitiateDeductionResponse {
    pub transaction_token: Uuid,
    pub commit_token: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ConfirmDeductionRequest {
    pub transaction_token: Uuid,
    pub commit_token: Uuid,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CancelDeductionRequest {
    pub transaction_token: Uuid,
}
