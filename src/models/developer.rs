use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub struct Developer {
    pub developer_uuid: Uuid,
    pub developer_name: String,
    pub successful_auths: i64,
    #[serde(skip_serializing)]
    pub risky_marks_available: i32,
    #[serde(skip_serializing)]
    pub total_risky_marks_earned: i32,
    #[serde(skip_serializing)]
    pub total_risky_marks_used: i32,
    pub last_auth_time: Option<DateTime<Utc>>,
    #[serde(skip_serializing)]
    pub auths_needed_for_next_mark: i32,
    pub create_time: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub rate_limit_per_second: i32,
    pub deduction_available: i32,
    pub deduction_limit: i32,
    #[serde(skip_serializing)]
    pub recovery_amount: i32,
    #[serde(skip_serializing)]
    pub recovery_interval_secs: i32,
    #[serde(skip_serializing)]
    pub last_recovery_time: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CreateDeveloperRequest {
    pub developer_uuid: Option<Uuid>,
    pub developer_name: String,
    #[serde(default)]
    pub successful_auths: Option<i64>,
    #[serde(skip_deserializing)]
    pub risky_marks_available: Option<i32>,
    #[serde(skip_deserializing)]
    pub total_risky_marks_earned: Option<i32>,
    #[serde(skip_deserializing)]
    pub total_risky_marks_used: Option<i32>,
    #[serde(default)]
    pub last_auth_time: Option<String>,
    #[serde(skip_deserializing)]
    pub auths_needed_for_next_mark: Option<i32>,
    #[serde(default)]
    pub create_time: Option<String>,
    #[serde(default)]
    pub deduction_available: Option<i32>,
    #[serde(default)]
    pub deduction_limit: Option<i32>,
    #[serde(default)]
    pub rate_limit_per_second: Option<i32>,
    #[serde(default)]
    pub recovery_amount: Option<i32>,
    #[serde(default)]
    pub recovery_interval_secs: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UpdateDeveloperRequest {
    pub developer_name: Option<String>,
    #[serde(default)]
    pub successful_auths: Option<i64>,
    #[serde(skip_deserializing)]
    pub risky_marks_available: Option<i32>,
    #[serde(skip_deserializing)]
    pub total_risky_marks_earned: Option<i32>,
    #[serde(skip_deserializing)]
    pub total_risky_marks_used: Option<i32>,
    #[serde(default)]
    pub last_auth_time: Option<String>,
    #[serde(skip_deserializing)]
    pub auths_needed_for_next_mark: Option<i32>,
    #[serde(default)]
    pub rate_limit_per_second: Option<i32>,
    #[serde(default)]
    pub deduction_available: Option<i32>,
    #[serde(default)]
    pub deduction_limit: Option<i32>,
    #[serde(default)]
    pub recovery_amount: Option<i32>,
    #[serde(default)]
    pub recovery_interval_secs: Option<i32>,
}
