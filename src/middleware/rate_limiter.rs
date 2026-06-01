use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse, web,
};
use std::future::{ready, Ready, Future};
use std::pin::Pin;
use std::task::{Context, Poll};
use redis::aio::ConnectionManager;
use crate::config::Config;
use crate::services::qps::QpsTracker;

pub struct RateLimiter;

impl<S, B> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RateLimiterService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterService { service }))
    }
}

pub struct RateLimiterService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RateLimiterService<S>
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
        let redis_conn = req.app_data::<web::Data<Option<ConnectionManager>>>().cloned();
        let qps_tracker = req.app_data::<web::Data<QpsTracker>>().cloned();
        let config = req.app_data::<web::Data<Config>>().cloned();
        let api_path = req.path().to_string();
        let dev_uuid = req.headers()
            .get("X-Developer-UUID")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        if let Some(ref tracker) = qps_tracker {
            tracker.record_request(&api_path, dev_uuid.as_deref());
        }

        let service = self.service.call(req);

        Box::pin(async move {
            if let Some(Some(mut conn)) = redis_conn.map(|d| d.get_ref().clone()) {
                let prefix = config.map(|c| c.redis_prefix.clone()).unwrap_or_else(|| "sl:uuid".to_string());
                let default_limit = 50000;

                let total_key = format!("{}:total_requests", prefix);
                let incr_expire_script = r#"
                    local key = KEYS[1]
                    local ttl = tonumber(ARGV[1])
                    local current = redis.call('INCR', key)
                    if current == 1 then
                        redis.call('EXPIRE', key, ttl)
                    end
                    return current
                "#;
                let _: Result<i64, _> = redis::cmd("EVAL")
                    .arg(incr_expire_script)
                    .arg(1)
                    .arg(&total_key)
                    .arg(86400)
                    .query_async(&mut conn)
                    .await;

                if let Some(ref uuid) = dev_uuid {
                    let key = format!("{}:ratelimit:{}", prefix, uuid);
                    let script = r#"
                        local key = KEYS[1]
                        local limit = tonumber(ARGV[1])
                        local current = tonumber(redis.call('GET', key) or '0')
                        if current >= limit then
                            return 0
                        end
                        redis.call('INCR', key)
                        redis.call('EXPIRE', key, 1)
                        return 1
                    "#;
                    let result: i64 = redis::cmd("EVAL")
                        .arg(script)
                        .arg(1)
                        .arg(&key)
                        .arg(default_limit)
                        .query_async(&mut conn)
                        .await
                        .unwrap_or(1);

                    if result == 0 {
                        return Ok(req.into_response(
                            HttpResponse::TooManyRequests().json(serde_json::json!({
                                "code": "RATE_LIMITED",
                                "message": "Rate limit exceeded"
                            })).map_into_right_body()
                        ).map_into_left_body());
                    }
                }
            }

            service.await
        })
    }
}