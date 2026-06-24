use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use redis::aio::ConnectionManager;
use crate::models::{
    InitiateDeductionRequest, ConfirmDeductionRequest, CancelDeductionRequest,
    ApiResponse
};
use crate::services::deduction;
use crate::config::Config;

pub async fn initiate(
    pg_pool: web::Data<PgPool>,
    redis_conn: web::Data<Option<ConnectionManager>>,
    config: web::Data<Config>,
    body: web::Json<InitiateDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut conn = redis_conn.get_ref().clone();
    let result = deduction::initiate_deduction(
        pg_pool.get_ref(),
        &mut conn,
        &config.redis_prefix,
        body.developer_uuid,
        body.amount,
        config.deduction_timeout_secs,
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

pub async fn confirm(
    pg_pool: web::Data<PgPool>,
    redis_conn: web::Data<Option<ConnectionManager>>,
    config: web::Data<Config>,
    body: web::Json<ConfirmDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut redis_conn = redis_conn.get_ref().clone();
    deduction::confirm_deduction(
        pg_pool.get_ref(),
        &mut redis_conn,
        &config.redis_prefix,
        body.into_inner(),
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Deduction confirmed")))
}

pub async fn cancel(
    pg_pool: web::Data<PgPool>,
    redis_conn: web::Data<Option<ConnectionManager>>,
    config: web::Data<Config>,
    body: web::Json<CancelDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut redis_conn = redis_conn.get_ref().clone();
    deduction::cancel_deduction(
        pg_pool.get_ref(),
        &mut redis_conn,
        &config.redis_prefix,
        body.into_inner(),
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Deduction cancelled")))
}

pub async fn get_transaction(
    pg_pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let tx_token = path.into_inner();
    let repo = crate::repositories::TransactionRepository::new(pg_pool.get_ref());
    let tx = repo.get_by_token(tx_token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(tx)))
}

pub async fn list_transactions(
    pg_pool: web::Data<PgPool>,
    _redis_conn: web::Data<Option<ConnectionManager>>,
    _config: web::Data<Config>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let page: i64 = query.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let page_size: i64 = query.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(20);
    let dev_uuid = query.get("developer_uuid")
        .and_then(|v| Uuid::parse_str(v).ok());
    let status = query.get("status").map(|s| s.as_str());

    let repo = crate::repositories::TransactionRepository::new(pg_pool.get_ref());
    let (txs, total) = repo.list_paginated(page, page_size, dev_uuid, status).await?;

    let resp = crate::models::PaginatedResponse::new(txs, total, page, page_size);
    Ok(HttpResponse::Ok().json(ApiResponse::success(resp)))
}