use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, Error};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::postgres::PgRow;
use sqlx::sqlite::SqliteRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub recovery_amount: i32,
    pub recovery_interval_secs: i32,
    pub last_recovery_time: Option<DateTime<Utc>>,
}

impl<'r> FromRow<'r, PgRow> for Developer {
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        Ok(Developer {
            developer_uuid: row.try_get("developer_uuid")?,
            developer_name: row.try_get("developer_name")?,
            successful_auths: row.try_get("successful_auths")?,
            risky_marks_available: row.try_get("risky_marks_available")?,
            total_risky_marks_earned: row.try_get("total_risky_marks_earned")?,
            total_risky_marks_used: row.try_get("total_risky_marks_used")?,
            last_auth_time: row.try_get("last_auth_time")?,
            auths_needed_for_next_mark: row.try_get("auths_needed_for_next_mark")?,
            create_time: row.try_get("create_time")?,
            updated_at: row.try_get("updated_at")?,
            rate_limit_per_second: row.try_get("rate_limit_per_second")?,
            deduction_available: row.try_get("deduction_available")?,
            deduction_limit: row.try_get("deduction_limit")?,
            recovery_amount: row.try_get("recovery_amount")?,
            recovery_interval_secs: row.try_get("recovery_interval_secs")?,
            last_recovery_time: row.try_get("last_recovery_time")?,
        })
    }
}

impl<'r> FromRow<'r, SqliteRow> for Developer {
    fn from_row(row: &SqliteRow) -> Result<Self, Error> {
        let uuid_str: String = row.try_get("developer_uuid")?;
        let developer_uuid = Uuid::parse_str(&uuid_str)
            .map_err(|e| Error::ColumnDecode {
                index: "developer_uuid".into(),
                source: Box::new(e),
            })?;

        Ok(Developer {
            developer_uuid,
            developer_name: row.try_get("developer_name")?,
            successful_auths: row.try_get("successful_auths")?,
            risky_marks_available: row.try_get("risky_marks_available")?,
            total_risky_marks_earned: row.try_get("total_risky_marks_earned")?,
            total_risky_marks_used: row.try_get("total_risky_marks_used")?,
            last_auth_time: row.try_get("last_auth_time")?,
            auths_needed_for_next_mark: row.try_get("auths_needed_for_next_mark")?,
            create_time: row.try_get("create_time")?,
            updated_at: row.try_get("updated_at")?,
            rate_limit_per_second: row.try_get("rate_limit_per_second")?,
            deduction_available: row.try_get("deduction_available")?,
            deduction_limit: row.try_get("deduction_limit")?,
            recovery_amount: row.try_get("recovery_amount")?,
            recovery_interval_secs: row.try_get("recovery_interval_secs")?,
            last_recovery_time: row.try_get("last_recovery_time")?,
        })
    }
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
