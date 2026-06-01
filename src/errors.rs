use actix_web::{HttpResponse, ResponseError};
use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    InternalError(String),
    #[allow(dead_code)]
    ServiceUnavailable(String),
    DatabaseError(String),
    RedisError(String),
    Unauthorized(String),
    Forbidden(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::BadRequest(msg) => write!(f, "Bad request: {}", msg),
            AppError::Conflict(msg) => write!(f, "Conflict: {}", msg),
            AppError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            AppError::ServiceUnavailable(msg) => write!(f, "Service unavailable: {}", msg),
            AppError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AppError::RedisError(msg) => write!(f, "Redis error: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            AppError::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let (status, code, message) = match self {
            AppError::NotFound(msg) => (actix_web::http::StatusCode::NOT_FOUND, "NOT_FOUND", msg),
            AppError::BadRequest(msg) => {
                (actix_web::http::StatusCode::BAD_REQUEST, "BAD_REQUEST", msg)
            }
            AppError::Conflict(msg) => (actix_web::http::StatusCode::CONFLICT, "CONFLICT", msg),
            AppError::InternalError(msg) => {
                (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", msg)
            }
            AppError::ServiceUnavailable(msg) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                msg,
            ),
            AppError::DatabaseError(msg) => {
                (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", msg)
            }
            AppError::RedisError(msg) => {
                (actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "REDIS_ERROR", msg)
            }
            AppError::Unauthorized(msg) => {
                (actix_web::http::StatusCode::UNAUTHORIZED, "UNAUTHORIZED", msg)
            }
            AppError::Forbidden(msg) => {
                (actix_web::http::StatusCode::FORBIDDEN, "FORBIDDEN", msg)
            }
        };

        HttpResponse::build(status).json(serde_json::json!({
            "code": code,
            "message": message,
        }))
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        log::error!("SQLx error: {:?}", err);
        AppError::DatabaseError(err.to_string())
    }
}

impl From<redis::RedisError> for AppError {
    fn from(err: redis::RedisError) -> Self {
        log::error!("Redis error: {:?}", err);
        AppError::RedisError(err.to_string())
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
