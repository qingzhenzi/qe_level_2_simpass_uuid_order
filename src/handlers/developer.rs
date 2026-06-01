use actix_web::{web, HttpResponse};
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use crate::config::Config;
use crate::errors::AppError;
use crate::models::*;

pub async fn create_developer(
    pg_pool: web::Data<PgPool>,
    body: web::Json<CreateDeveloperRequest>,
) -> Result<HttpResponse, AppError> {
    let dev_uuid = body.developer_uuid.unwrap_or_else(Uuid::new_v4);
    let now = Utc::now();

    let last_auth = body.last_auth_time.as_ref()
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .map(|t| t.and_utc());

    let create_time = body.create_time.as_ref()
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .map(|t| t.and_utc())
        .unwrap_or(now);

    sqlx::query(
        r#"INSERT INTO developers 
           (developer_uuid, developer_name, successful_auths, risky_marks_available,
            total_risky_marks_earned, total_risky_marks_used, last_auth_time,
            auths_needed_for_next_mark, create_time, updated_at, deduction_available, 
            deduction_limit, rate_limit_per_second, recovery_amount, recovery_interval_secs)
           VALUES ($1, $2, $3, 0, 0, 0, $4, 0, $5, $6, $7, $8, $9, $10, $11)"#
    )
    .bind(dev_uuid)
    .bind(&body.developer_name)
    .bind(body.successful_auths.unwrap_or(0))
    .bind(last_auth)
    .bind(create_time)
    .bind(now)
    .bind(body.deduction_available.unwrap_or(0))
    .bind(body.deduction_limit.unwrap_or(1000))
    .bind(body.rate_limit_per_second.unwrap_or(100))
    .bind(body.recovery_amount.unwrap_or(10))
    .bind(body.recovery_interval_secs.unwrap_or(60))
    .execute(pg_pool.get_ref())
    .await?;

    log::info!("Developer created: {} ({})", body.developer_name, dev_uuid);
    Ok(HttpResponse::Created().json(ApiResponse::success(serde_json::json!({
        "developer_uuid": dev_uuid
    }))))
}

pub async fn list_developers(
    pg_pool: web::Data<PgPool>,
    redis_conn: web::Data<Option<ConnectionManager>>,
    config: web::Data<Config>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let page: i64 = query.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let page_size: i64 = query.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(20);
    let search = query.get("search");
    let offset = (page - 1) * page_size;

    let cache_key = format!(
        "{}:dev:list:{}:{}:{}",
        config.redis_prefix,
        page,
        page_size,
        search.as_ref().map(|s| s.as_str()).unwrap_or("")
    );

    let mut redis_conn = redis_conn.get_ref().clone();
    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        if let Ok(cached) = conn.get::<_, Option<String>>(&cache_key).await {
            if let Some(data) = cached {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                    return Ok(HttpResponse::Ok().json(ApiResponse::success(v)));
                }
            }
        }
    }

    let (developers, total) = if let Some(keyword) = search {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM developers WHERE developer_name ILIKE $1"
        )
        .bind(format!("%{}%", keyword))
        .fetch_one(pg_pool.get_ref())
        .await?;

        let devs = sqlx::query_as::<_, Developer>(
            r#"SELECT * FROM developers
               WHERE developer_name ILIKE $1
               ORDER BY create_time DESC
               LIMIT $2 OFFSET $3"#
        )
        .bind(format!("%{}%", keyword))
        .bind(page_size)
        .bind(offset)
        .fetch_all(pg_pool.get_ref())
        .await?;

        (devs, count.0)
    } else {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM developers")
            .fetch_one(pg_pool.get_ref())
            .await?;

        let devs = sqlx::query_as::<_, Developer>(
            r#"SELECT * FROM developers
               ORDER BY create_time DESC
               LIMIT $1 OFFSET $2"#
        )
        .bind(page_size)
        .bind(offset)
        .fetch_all(pg_pool.get_ref())
        .await?;

        (devs, count.0)
    };

    let resp = PaginatedResponse::new(developers, total, page, page_size);

    if let Some(ref mut conn) = redis_conn {
        use redis::AsyncCommands;
        let _: Result<(), _> = conn.set_ex(&cache_key, serde_json::to_string(&resp).unwrap_or_default(), 5).await;
    }

    Ok(HttpResponse::Ok().json(ApiResponse::success(resp)))
}

pub async fn get_developer(
    pg_pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let dev_uuid = path.into_inner();
    let dev = sqlx::query_as::<_, Developer>(
        "SELECT * FROM developers WHERE developer_uuid = $1"
    )
    .bind(dev_uuid)
    .fetch_optional(pg_pool.get_ref())
    .await?
    .ok_or_else(|| AppError::NotFound("Developer not found".into()))?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(dev)))
}

pub async fn update_developer(
    pg_pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateDeveloperRequest>,
) -> Result<HttpResponse, AppError> {
    let dev_uuid = path.into_inner();

    let _existing = sqlx::query_as::<_, Developer>(
        "SELECT * FROM developers WHERE developer_uuid = $1"
    )
    .bind(dev_uuid)
    .fetch_optional(pg_pool.get_ref())
    .await?
    .ok_or_else(|| AppError::NotFound("Developer not found".into()))?;

    let last_auth = body.last_auth_time.as_ref()
        .and_then(|s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .map(|t| t.and_utc());

    sqlx::query(
        r#"UPDATE developers SET 
           developer_name = COALESCE($1, developer_name),
           successful_auths = COALESCE($2, successful_auths),
           last_auth_time = COALESCE($3, last_auth_time),
           rate_limit_per_second = COALESCE($4, rate_limit_per_second),
           deduction_available = COALESCE($5, deduction_available),
           deduction_limit = COALESCE($6, deduction_limit),
           recovery_amount = COALESCE($7, recovery_amount),
           recovery_interval_secs = COALESCE($8, recovery_interval_secs),
           updated_at = NOW()
           WHERE developer_uuid = $9"#
    )
    .bind(body.developer_name.as_ref())
    .bind(body.successful_auths)
    .bind(last_auth)
    .bind(body.rate_limit_per_second)
    .bind(body.deduction_available)
    .bind(body.deduction_limit)
    .bind(body.recovery_amount)
    .bind(body.recovery_interval_secs)
    .bind(dev_uuid)
    .execute(pg_pool.get_ref())
    .await?;

    log::info!("Developer updated: {}", dev_uuid);
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Developer updated")))
}

pub async fn delete_developer(
    pg_pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let dev_uuid = path.into_inner();

    let result = sqlx::query("DELETE FROM developers WHERE developer_uuid = $1")
        .bind(dev_uuid)
        .execute(pg_pool.get_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Developer not found".into()));
    }

    sqlx::query("DELETE FROM deduction_transactions WHERE developer_uuid = $1")
        .bind(dev_uuid)
        .execute(pg_pool.get_ref())
        .await?;

    log::info!("Developer deleted: {}", dev_uuid);
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Developer deleted")))
}