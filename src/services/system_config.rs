use sqlx::PgPool;
use crate::models::system_config::{SystemConfig, CreateSystemConfigRequest, UpdateSystemConfigRequest};
use crate::errors::AppError;
use chrono::Utc;

pub async fn get_config(pg_pool: &PgPool, key: &str) -> Result<Option<SystemConfig>, AppError> {
    let config = sqlx::query_as::<_, SystemConfig>(
        "SELECT * FROM system_configs WHERE config_key = $1"
    )
    .bind(key)
    .fetch_optional(pg_pool)
    .await?;
    Ok(config)
}

pub async fn get_all_configs(pg_pool: &PgPool) -> Result<Vec<SystemConfig>, AppError> {
    let configs = sqlx::query_as::<_, SystemConfig>(
        "SELECT * FROM system_configs ORDER BY config_key"
    )
    .fetch_all(pg_pool)
    .await?;
    Ok(configs)
}

pub async fn create_config(
    pg_pool: &PgPool,
    req: CreateSystemConfigRequest,
) -> Result<SystemConfig, AppError> {
    let config = sqlx::query_as::<_, SystemConfig>(
        "INSERT INTO system_configs (config_key, config_value, description, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5) RETURNING *"
    )
    .bind(req.config_key)
    .bind(req.config_value)
    .bind(req.description)
    .bind(Utc::now())
    .bind(Utc::now())
    .fetch_one(pg_pool)
    .await?;
    Ok(config)
}

pub async fn update_config(
    pg_pool: &PgPool,
    key: &str,
    req: UpdateSystemConfigRequest,
) -> Result<SystemConfig, AppError> {
    let config = sqlx::query_as::<_, SystemConfig>(
        "UPDATE system_configs SET config_value = $1, description = $2, updated_at = $3 
         WHERE config_key = $4 RETURNING *"
    )
    .bind(req.config_value)
    .bind(req.description)
    .bind(Utc::now())
    .bind(key)
    .fetch_one(pg_pool)
    .await?;
    Ok(config)
}

pub async fn delete_config(pg_pool: &PgPool, key: &str) -> Result<(), AppError> {
    let result = sqlx::query(
        "DELETE FROM system_configs WHERE config_key = $1"
    )
    .bind(key)
    .execute(pg_pool)
    .await?;
    
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Config with key '{}' not found", key)));
    }
    Ok(())
}

pub async fn get_allowed_useragents(pg_pool: &PgPool) -> Result<Vec<String>, AppError> {
    if let Some(config) = get_config(pg_pool, "deduction_allowed_useragents").await? {
        Ok(config.config_value.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    } else {
        Ok(Vec::new())
    }
}

pub async fn get_api_token(pg_pool: &PgPool) -> Result<Option<String>, AppError> {
    if let Some(config) = get_config(pg_pool, "deduction_api_token").await? {
        if config.config_value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(config.config_value))
        }
    } else {
        Ok(None)
    }
}
