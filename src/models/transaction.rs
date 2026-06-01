use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeductionTransaction {
    pub id: i64,
    pub developer_uuid: Uuid,
    pub transaction_token: Uuid,
    pub amount: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub commit_token: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct InitiateDeductionRequest {
    pub developer_uuid: Uuid,
    pub amount: i32,
}

#[derive(Debug, Serialize)]
pub struct InitiateDeductionResponse {
    pub transaction_token: Uuid,
    pub commit_token: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmDeductionRequest {
    pub transaction_token: Uuid,
    pub commit_token: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CancelDeductionRequest {
    pub transaction_token: Uuid,
}
