use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, web,
};
use std::future::{ready, Ready, Future};
use std::pin::Pin;
use std::task::{Context, Poll};
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use crate::config::Config;
use crate::errors::AppError;

const DEDUCTION_PATHS: [&str; 3] = [
    "/api/deductions/initiate",
    "/api/deductions/confirm",
    "/api/deductions/cancel",
];

pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService { service }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let pool = req.app_data::<web::Data<PgPool>>().cloned();
        let redis_conn = req.app_data::<web::Data<Option<ConnectionManager>>>().cloned();
        let config = req.app_data::<web::Data<Config>>().cloned();
        let path = req.path().to_string();
        let is_deduction_path = DEDUCTION_PATHS.contains(&path.as_str());

        if !is_deduction_path {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }

        let validate = async move {
            if let (Some(pool), Some(_redis_conn), Some(cfg)) = (pool, redis_conn, config) {
                validate_request(&req, &pool, &cfg).await?;
            }
            Ok(())
        };

        let service = self.service.call(req);

        Box::pin(async move {
            validate.await?;
            service.await
        })
    }
}

async fn validate_request(req: &ServiceRequest, pool: &PgPool, cfg: &Config) -> Result<(), AppError> {
    let user_agent = req.headers()
        .get("User-Agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let api_token = req.headers()
        .get("X-API-Token")
        .and_then(|h| h.to_str().ok());

    let env_allowed_user_agents = &cfg.deduction_allowed_useragents;
    let env_api_token = &cfg.deduction_api_token;

    let has_env_config = !env_allowed_user_agents.is_empty() || env_api_token.is_some();

    if has_env_config {
        let user_agent_ok = env_allowed_user_agents.is_empty() ||
            env_allowed_user_agents.iter().any(|ua| user_agent.contains(ua));
        let token_ok = env_api_token.is_none() ||
            env_api_token.as_ref().map_or(false, |t| api_token == Some(t.as_str()));

        if !user_agent_ok {
            return Err(AppError::Forbidden("Invalid User-Agent".into()));
        }
        if !token_ok {
            return Err(AppError::Unauthorized("Invalid API Token".into()));
        }
        return Ok(());
    }

    Ok(())
}