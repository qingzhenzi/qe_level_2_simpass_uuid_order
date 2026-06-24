use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use chrono::Utc;
use serde_json::Value;
use crate::errors::AppError;
use crate::models::*;
use crate::services::qps::QpsTracker;
use crate::config::Config;

const QPS_CACHE_TTL_SECS: u64 = 30;

pub async fn current_qps(
    qps_tracker: web::Data<QpsTracker>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let api_path = query.get("api_path").map(|s| s.as_str()).unwrap_or("/api");
    let qps = qps_tracker.get_current_qps(api_path).await;
    let qps_1m = qps_tracker.get_qps_since(api_path, 60).await;
    let qps_5m = qps_tracker.get_qps_since(api_path, 300).await;

    Ok(HttpResponse::Ok().json(ApiResponse::success(serde_json::json!({
        "current_qps": qps,
        "avg_qps_1m": qps_1m,
        "avg_qps_5m": qps_5m,
    }))))
}

pub async fn qps_history(
    pg_pool: web::Data<PgPool>,
    redis_conn: web::Data<Option<ConnectionManager>>,
    config: web::Data<Config>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let api_path = query.get("api_path").map(|s| s.as_str()).unwrap_or("");
    let minutes: i64 = query.get("minutes").and_then(|v| v.parse().ok()).unwrap_or(10);
    let page_size: i64 = query.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(50);
    let page_size = page_size.min(200);
    let cutoff = Utc::now() - chrono::Duration::minutes(minutes);

    let records = {
        let mut redis_conn = redis_conn.get_ref().clone();

        let cache_key = if api_path.is_empty() {
            format!("{}:qps:history:{}:{}", config.redis_prefix, minutes, page_size)
        } else {
            format!("{}:qps:history:{}:{}:{}", config.redis_prefix, minutes, page_size, api_path)
        };

        if let Some(ref mut conn) = redis_conn {
            use redis::AsyncCommands;
            if let Ok(cached) = conn.get::<_, String>(&cache_key).await {
                if let Ok(v) = serde_json::from_str::<Value>(&cached) {
                    return Ok(HttpResponse::Ok().json(ApiResponse::success(v)));
                }
            }
        }

        let records = if api_path.is_empty() {
            sqlx::query_as::<_, QpsRecord>(
                "SELECT * FROM qps_records WHERE recorded_at >= $1 ORDER BY recorded_at DESC LIMIT $2"
            )
            .bind(cutoff)
            .bind(page_size)
            .fetch_all(pg_pool.get_ref())
            .await?
        } else {
            sqlx::query_as::<_, QpsRecord>(
                "SELECT * FROM qps_records WHERE recorded_at >= $1 AND api_path = $2 ORDER BY recorded_at DESC LIMIT $3"
            )
            .bind(cutoff)
            .bind(api_path)
            .bind(page_size)
            .fetch_all(pg_pool.get_ref())
            .await?
        };

        if let Some(ref mut conn) = redis_conn {
            use redis::AsyncCommands;
            let json = serde_json::to_string(&records).unwrap_or_default();
            let _: Result<(), _> = conn.set_ex(&cache_key, json, QPS_CACHE_TTL_SECS).await;
        }

        records
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(records)))
}

pub async fn qps_stats(
    redis_conn: web::Data<Option<ConnectionManager>>,
    pg_pool: web::Data<PgPool>,
    config: web::Data<Config>,
    tracker: web::Data<QpsTracker>,
) -> Result<HttpResponse, AppError> {
    let cache_key = format!("{}:qps:stats", config.redis_prefix);
    let total_key = format!("{}:total_requests", config.redis_prefix);
    let ttl = QPS_CACHE_TTL_SECS;

    // total_requests: from Redis or PG
    let total: i64 = {
        let mut conn = redis_conn.get_ref().clone();
        if let Some(ref mut conn) = conn {
            use redis::AsyncCommands;
            let from_redis: Option<i64> = conn.get(&total_key).await.unwrap_or(None);
            match from_redis {
                Some(v) => v,
                None => {
                    let from_db = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM request_logs")
                        .fetch_one(pg_pool.get_ref())
                        .await
                        .map(|r| r.0)
                        .unwrap_or(0);
                    let _ = conn.set_ex::<_, _, ()>(&total_key, from_db, 3600).await;
                    from_db
                }
            }
        } else {
            sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM request_logs")
                .fetch_one(pg_pool.get_ref())
                .await
                .map(|r| r.0)
                .unwrap_or(0)
        }
    };

    // Check cache
    {
        let mut conn = redis_conn.get_ref().clone();
        if let Some(ref mut conn) = conn {
            use redis::AsyncCommands;
            if let Ok(cached) = conn.get::<_, String>(&cache_key).await {
                if let Ok(v) = serde_json::from_str::<Value>(&cached) {
                    return Ok(HttpResponse::Ok().json(ApiResponse::success(v)));
                }
            }
        }
    }

    // Distributed lock: only one instance computes stats at a time
    // Uses Redis SET NX so across N instances only one does the work
    let lock_key = format!("{}:qps:stats:lock", config.redis_prefix);
    {
        let mut conn = redis_conn.get_ref().clone();
        let got_lock = if let Some(ref mut conn) = conn {
            use redis::AsyncCommands;
            match conn.set_nx(&lock_key, "1").await {
                Ok(true) => {
                    let _: Result<(), _> = conn.expire(&lock_key, ttl as i64).await;
                    true
                }
                _ => false,
            }
        } else {
            true // no Redis — always compute
        };

        if got_lock {
            // Re-check cache under lock
            let mut conn = redis_conn.get_ref().clone();
            if let Some(ref mut conn) = conn {
                use redis::AsyncCommands;
                if let Ok(cached) = conn.get::<_, String>(&cache_key).await {
                    if let Ok(v) = serde_json::from_str::<Value>(&cached) {
                        let _: Result<(), _> = conn.del(&lock_key).await;
                        return Ok(HttpResponse::Ok().json(ApiResponse::success(v)));
                    }
                }
            }

            let current_qps = tracker.get_current_qps("/").await;
            let avg_1m = tracker.get_avg_qps_across_paths(60).await;
            let avg_5m = tracker.get_avg_qps_across_paths(300).await;
            let avg_1h = tracker.get_avg_qps_across_paths(3600).await;

            let api_stats_raw = tracker.get_all_path_counts_since(60).await;
            let api_stats: Vec<ApiQpsStats> = api_stats_raw
                .into_iter()
                .map(|(path, count)| ApiQpsStats {
                    api_path: path,
                    count,
                    qps: count as f64 / 60.0,
                })
                .collect();

            let response = QpsStatsResponse {
                current_qps,
                avg_qps_1m: avg_1m,
                avg_qps_5m: avg_5m,
                avg_qps_1h: avg_1h,
                total_requests: total,
                api_stats,
                aggregation_errors: tracker.aggregation_errors(),
            };

            {
                let mut conn = redis_conn.get_ref().clone();
                if let Some(ref mut conn) = conn {
                    use redis::AsyncCommands;
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _: Result<(), _> = conn.set_ex(&cache_key, json, ttl).await;
                    }
                    let _: Result<(), _> = conn.del(&lock_key).await;
                }
            }

            return Ok(HttpResponse::Ok().json(ApiResponse::success(response)));
        }
    }

    // Didn't get lock — another instance is computing. Return a minimal response
    let response = QpsStatsResponse {
        current_qps: 0,
        avg_qps_1m: 0.0,
        avg_qps_5m: 0.0,
        avg_qps_1h: 0.0,
        total_requests: total,
        api_stats: vec![],
        aggregation_errors: tracker.aggregation_errors(),
    };
    Ok(HttpResponse::Ok().json(ApiResponse::success(response)))
}
