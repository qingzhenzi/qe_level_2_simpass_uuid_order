use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemConfig {
    pub id: i64,
    pub config_key: String,
    pub config_value: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSystemConfigRequest {
    pub config_value: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSystemConfigRequest {
    pub config_key: String,
    pub config_value: String,
    pub description: Option<String>,
}
