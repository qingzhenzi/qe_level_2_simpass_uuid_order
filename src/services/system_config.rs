use crate::db::DbPool;
use crate::models::system_config::{SystemConfig, CreateSystemConfigRequest, UpdateSystemConfigRequest};
use crate::errors::AppError;
use chrono::Utc;

pub async fn get_config(db: &DbPool, key: &str) -> Result<Option<SystemConfig>, AppError> {
    let config = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1"
            )
            .bind(key)
            .fetch_optional(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1"
            )
            .bind(key)
            .fetch_optional(sq)
            .await?
        }
    };
    Ok(config)
}

pub async fn get_all_configs(db: &DbPool) -> Result<Vec<SystemConfig>, AppError> {
    let configs = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, SystemConfig>("SELECT * FROM system_configs ORDER BY config_key")
                .fetch_all(pg)
                .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, SystemConfig>("SELECT * FROM system_configs ORDER BY config_key")
                .fetch_all(sq)
                .await?
        }
    };
    Ok(configs)
}

pub async fn create_config(
    db: &DbPool,
    req: CreateSystemConfigRequest,
) -> Result<SystemConfig, AppError> {
    let now = Utc::now().to_rfc3339();
    match db {
        DbPool::Postgres(pg) => {
            sqlx::query(
                "INSERT INTO system_configs (config_key, config_value, description, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(&req.config_key)
            .bind(&req.config_value)
            .bind(&req.description)
            .bind(Utc::now())
            .bind(Utc::now())
            .execute(pg)
            .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query(
                "INSERT INTO system_configs (config_key, config_value, description, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(&req.config_key)
            .bind(&req.config_value)
            .bind(&req.description)
            .bind(&now)
            .bind(&now)
            .execute(sq)
            .await?;
        }
    }

    // Fetch the inserted row
    let config = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1 ORDER BY id DESC LIMIT 1"
            )
            .bind(&req.config_key)
            .fetch_one(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1 ORDER BY id DESC LIMIT 1"
            )
            .bind(&req.config_key)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(config)
}

pub async fn update_config(
    db: &DbPool,
    key: &str,
    req: UpdateSystemConfigRequest,
) -> Result<SystemConfig, AppError> {
    let now = Utc::now().to_rfc3339();
    match db {
        DbPool::Postgres(pg) => {
            sqlx::query(
                "UPDATE system_configs SET config_value = $1, description = $2, updated_at = $3 \
                 WHERE config_key = $4"
            )
            .bind(&req.config_value)
            .bind(&req.description)
            .bind(Utc::now())
            .bind(key)
            .execute(pg)
            .await?;
        }
        DbPool::Sqlite(sq) => {
            sqlx::query(
                "UPDATE system_configs SET config_value = $1, description = $2, updated_at = $3 \
                 WHERE config_key = $4"
            )
            .bind(&req.config_value)
            .bind(&req.description)
            .bind(&now)
            .bind(key)
            .execute(sq)
            .await?;
        }
    }

    let config = match db {
        DbPool::Postgres(pg) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1"
            )
            .bind(key)
            .fetch_one(pg)
            .await?
        }
        DbPool::Sqlite(sq) => {
            sqlx::query_as::<_, SystemConfig>(
                "SELECT * FROM system_configs WHERE config_key = $1"
            )
            .bind(key)
            .fetch_one(sq)
            .await?
        }
    };
    Ok(config)
}

pub async fn delete_config(db: &DbPool, key: &str) -> Result<(), AppError> {
    let affected = match db {
        DbPool::Postgres(pg) => {
            sqlx::query("DELETE FROM system_configs WHERE config_key = $1")
                .bind(key)
                .execute(pg)
                .await?
                .rows_affected()
        }
        DbPool::Sqlite(sq) => {
            sqlx::query("DELETE FROM system_configs WHERE config_key = $1")
                .bind(key)
                .execute(sq)
                .await?
                .rows_affected()
        }
    };

    if affected == 0 {
        return Err(AppError::NotFound(format!("Config with key '{}' not found", key)));
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn get_allowed_useragents(db: &DbPool) -> Result<Vec<String>, AppError> {
    if let Some(config) = get_config(db, "deduction_allowed_useragents").await? {
        Ok(config.config_value.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    } else {
        Ok(Vec::new())
    }
}

#[allow(dead_code)]
pub async fn get_api_token(db: &DbPool) -> Result<Option<String>, AppError> {
    if let Some(config) = get_config(db, "deduction_api_token").await? {
        if config.config_value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(config.config_value))
        }
    } else {
        Ok(None)
    }
}
