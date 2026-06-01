use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use crate::models::{ApiResponse, system_config::{SystemConfig, CreateSystemConfigRequest, UpdateSystemConfigRequest}};
use crate::services::system_config;
use crate::errors::AppError;

pub async fn get_configs(pg_pool: web::Data<PgPool>) -> Result<HttpResponse, AppError> {
    let configs = system_config::get_all_configs(pg_pool.get_ref()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(configs)))
}

pub async fn get_config(pg_pool: web::Data<PgPool>, path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let key = path.into_inner();
    let config = system_config::get_config(pg_pool.get_ref(), &key)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Config '{}' not found", key)))?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(config)))
}

pub async fn create_config(
    pg_pool: web::Data<PgPool>,
    body: web::Json<CreateSystemConfigRequest>,
) -> Result<HttpResponse, AppError> {
    let config = system_config::create_config(pg_pool.get_ref(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(ApiResponse::success(config)))
}

pub async fn update_config(
    pg_pool: web::Data<PgPool>,
    path: web::Path<String>,
    body: web::Json<UpdateSystemConfigRequest>,
) -> Result<HttpResponse, AppError> {
    let key = path.into_inner();
    let config = system_config::update_config(pg_pool.get_ref(), &key, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::success(config)))
}

pub async fn delete_config(pg_pool: web::Data<PgPool>, path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let key = path.into_inner();
    system_config::delete_config(pg_pool.get_ref(), &key).await?;
    Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_msg("Config deleted")))
}
