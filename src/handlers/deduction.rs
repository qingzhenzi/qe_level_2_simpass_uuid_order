use actix_web::{web, HttpResponse};
use uuid::Uuid;
use crate::db::DbPool;
use crate::models::{
    InitiateDeductionRequest, ConfirmDeductionRequest, CancelDeductionRequest,
    ApiResponse
};
use crate::services::deduction;
use crate::config::Config;
use crate::cache::backend::CacheBackend;

pub async fn initiate(
    db: web::Data<DbPool>,
    cache: web::Data<CacheBackend>,
    config: web::Data<Config>,
    body: web::Json<InitiateDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut cache = cache.get_ref().clone();
    let result = deduction::initiate_deduction(
        db.get_ref(),
        &mut cache,
        body.developer_uuid,
        body.amount,
        config.deduction_timeout_secs,
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::success(result)))
}

pub async fn confirm(
    db: web::Data<DbPool>,
    cache: web::Data<CacheBackend>,
    body: web::Json<ConfirmDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut cache = cache.get_ref().clone();
    deduction::confirm_deduction(
        db.get_ref(),
        &mut cache,
        body.into_inner(),
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Deduction confirmed")))
}

pub async fn cancel(
    db: web::Data<DbPool>,
    cache: web::Data<CacheBackend>,
    body: web::Json<CancelDeductionRequest>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let mut cache = cache.get_ref().clone();
    deduction::cancel_deduction(
        db.get_ref(),
        &mut cache,
        body.into_inner(),
    ).await?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Deduction cancelled")))
}

pub async fn get_transaction(
    db: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let tx_token = path.into_inner();
    let repo = crate::repositories::TransactionRepository::new(db.get_ref().clone());
    let tx = repo.get_by_token(tx_token).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(tx)))
}

pub async fn list_transactions(
    db: web::Data<DbPool>,
    _cache: web::Data<CacheBackend>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, crate::errors::AppError> {
    let page: i64 = query.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let page_size: i64 = query.get("page_size").and_then(|v| v.parse().ok()).unwrap_or(20);
    let dev_uuid = query.get("developer_uuid")
        .and_then(|v| Uuid::parse_str(v).ok());
    let status = query.get("status").map(|s| s.as_str());

    let repo = crate::repositories::TransactionRepository::new(db.get_ref().clone());
    let (txs, total) = repo.list_paginated(page, page_size, dev_uuid, status).await?;

    let resp = crate::models::PaginatedResponse::new(txs, total, page, page_size);
    Ok(HttpResponse::Ok().json(ApiResponse::success(resp)))
}
