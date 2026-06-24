use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QpsRecord {
    pub id: i64,
    pub recorded_at: DateTime<Utc>,
    pub total_qps: i32,
    pub api_path: String,
    pub developer_uuid: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct QpsStatsResponse {
    pub current_qps: i64,
    pub avg_qps_1m: f64,
    pub avg_qps_5m: f64,
    pub avg_qps_1h: f64,
    pub total_requests: i64,
    pub api_stats: Vec<ApiQpsStats>,
    pub aggregation_errors: u64,
}

#[derive(Debug, Serialize)]
pub struct ApiQpsStats {
    pub api_path: String,
    pub count: i64,
    pub qps: f64,
}
