mod cache;
mod config;
mod db;
mod errors;
mod handlers;
mod middleware;
mod models;
mod repositories;
mod services;
mod tasks;

use actix_web::{web, App, HttpServer, HttpResponse, middleware as actix_middleware};
use actix_web::middleware::from_fn;
use actix_cors::Cors;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::config::Config;

struct AppState {
    #[allow(dead_code)]
    health_checker: db::health::HealthChecker,
    service_healthy: Arc<AtomicBool>,
}

async fn health_check(state: web::Data<AppState>) -> HttpResponse {
    if state.service_healthy.load(Ordering::Acquire) {
        HttpResponse::Ok().json(serde_json::json!({"status": "healthy"}))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({"status": "unhealthy"}))
    }
}

async fn not_found() -> HttpResponse {
    HttpResponse::NotFound().json(serde_json::json!({
        "code": "NOT_FOUND",
        "message": "Resource not found"
    }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();

    let cfg = config::Config::from_env();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(&cfg.log_level)
    )
    .format_timestamp_millis()
    .init();

    log::info!("Starting server on {}:{}, log_level: {}",
        cfg.server_host, cfg.server_port, cfg.log_level);

    let pg_pool = db::pg_pool::create_pool(&cfg)
        .await
        .expect("Failed to create PostgreSQL pool");

    log::info!("Running database migrations...");

    db::migrations::run_migrations(&pg_pool).await;

    let redis_conn = match db::redis_pool::create_connection_manager(&cfg).await {
        Ok(conn) => {
            log::info!("Redis connection manager established");
            Some(conn)
        }
        Err(e) => {
            log::warn!("Redis not available (server will run in degraded mode): {}", e);
            None
        }
    };

    let health_checker = db::health::HealthChecker::new(pg_pool.clone(), redis_conn.clone());
    let service_healthy = Arc::new(AtomicBool::new(true));
    let service_healthy_clone = service_healthy.clone();
    let health_checker_for_health = health_checker.clone();
    let health_checker_for_start = health_checker.clone();

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let healthy = health_checker_for_health.is_healthy();
            service_healthy_clone.store(healthy, Ordering::Release);
        }
    });

    health_checker_for_start.start_health_check();

    let qps_tracker = services::qps::QpsTracker::new(
        pg_pool.clone(),
        redis_conn.clone(),
        cfg.redis_prefix.clone(),
    );
    qps_tracker.clone().start_aggregation();
    // NOTE: cleanup is handled by Redis TTL (7200s on each counter key)

    tasks::start_expiration_task(pg_pool.clone(), redis_conn.clone(), cfg.redis_prefix.clone());

    let bind_addr = format!("{}:{}", cfg.server_host, cfg.server_port);
    log::info!("Listening on {}", bind_addr);

    let cfg_data = cfg.clone();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        let service_healthy = service_healthy.clone();

        App::new()
            .wrap(actix_middleware::Logger::default())
            .wrap(cors)
            .wrap(from_fn(admin_auth_middleware))
            .wrap(from_fn(rate_limit_middleware))
            .app_data(web::Data::new(pg_pool.clone()))
            .app_data(web::Data::new(redis_conn.clone() as Option<ConnectionManager>))
            .app_data(web::Data::new(cfg_data.clone()))
            .app_data(web::Data::new(qps_tracker.clone()))
            .app_data(web::Data::new(AppState {
                health_checker: health_checker.clone(),
                service_healthy: service_healthy.clone(),
            }))
            .route("/health", web::get().to(health_check))
            .route("/api/developers", web::post().to(handlers::developer::create_developer))
            .route("/api/developers", web::get().to(handlers::developer::list_developers))
            .route("/api/developers/{uuid}", web::get().to(handlers::developer::get_developer))
            .route("/api/developers/{uuid}", web::put().to(handlers::developer::update_developer))
            .route("/api/developers/{uuid}", web::delete().to(handlers::developer::delete_developer))
            .route("/api/deductions/initiate", web::post().to(handlers::deduction::initiate))
            .route("/api/deductions/confirm", web::post().to(handlers::deduction::confirm))
            .route("/api/deductions/cancel", web::post().to(handlers::deduction::cancel))
            .route("/api/deductions/transactions", web::get().to(handlers::deduction::list_transactions))
            .route("/api/deductions/transactions/{token}", web::get().to(handlers::deduction::get_transaction))
            .route("/api/qps/current", web::get().to(handlers::qps::current_qps))
            .route("/api/qps/history", web::get().to(handlers::qps::qps_history))
            .route("/api/qps/stats", web::get().to(handlers::qps::qps_stats))
            .route("/api/system/configs", web::get().to(handlers::system_config::get_configs))
            .route("/api/system/configs", web::post().to(handlers::system_config::create_config))
            .route("/api/system/configs/{key}", web::get().to(handlers::system_config::get_config))
            .route("/api/system/configs/{key}", web::put().to(handlers::system_config::update_config))
            .route("/api/system/configs/{key}", web::delete().to(handlers::system_config::delete_config))
            .default_service(web::route().to(not_found))
    })
    .bind(&bind_addr)?
    .run()
    .await
}

// ── 中间件函数 ──────────────────────────────────────────────

async fn admin_auth_middleware(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody + 'static>,
) -> Result<actix_web::dev::ServiceResponse<actix_web::body::BoxBody>, actix_web::Error> {
    use actix_web::HttpResponse;
    let path = req.path();
    const ADMIN_PATHS: [&str; 6] = [
        "/api/developers",
        "/api/deductions/transactions",
        "/api/qps/current",
        "/api/qps/history",
        "/api/qps/stats",
        "/api/system/configs",
    ];

    let is_admin = ADMIN_PATHS.iter().any(|&p| path.starts_with(p));

    if is_admin {
        let config = req.app_data::<actix_web::web::Data<Config>>();
        if let Some(cfg) = config {
            if let Some(env_token) = &cfg.admin_api_token {
                let token_ok = req.headers()
                    .get("X-Admin-Token")
                    .and_then(|h| h.to_str().ok())
                    == Some(env_token.as_str());

                if !token_ok {
                    let response = HttpResponse::Unauthorized()
                        .json(serde_json::json!({
                            "code": "UNAUTHORIZED",
                            "message": "Invalid Admin Token"
                        }));
                    let (req_parts, _) = req.into_parts();
                    return Ok(actix_web::dev::ServiceResponse::new(req_parts, response.map_into_boxed_body()));
                }
            }
        }
    }

    let res = next.call(req).await?;
    Ok(res.map_into_boxed_body())
}

async fn rate_limit_middleware(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody + 'static>,
) -> Result<actix_web::dev::ServiceResponse<actix_web::body::BoxBody>, actix_web::Error> {
    use actix_web::HttpResponse;

    let redis_conn = req.app_data::<actix_web::web::Data<Option<ConnectionManager>>>();
    let qps_tracker = req.app_data::<actix_web::web::Data<crate::services::qps::QpsTracker>>();
    let redis_prefix = req
        .app_data::<actix_web::web::Data<Config>>()
        .map(|c| c.redis_prefix.clone())
        .unwrap_or_else(|| "sl:uuid".to_string());
    let default_limit = 50000;

    let api_path = req.path().to_string();
    let dev_uuid = req
        .headers()
        .get("X-Developer-UUID")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref tracker) = qps_tracker {
        tracker.record_request(&api_path, dev_uuid.as_deref()).await;
    }

    if let Some(Some(conn)) = redis_conn.map(|d| d.get_ref()) {
        let mut conn = conn.clone();
        let total_key = format!("{}:total_requests", redis_prefix);
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
            let key = format!("{}:ratelimit:{}", redis_prefix, uuid);
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
                .unwrap_or(0);

            if result == 0 {
                let response = HttpResponse::TooManyRequests()
                    .json(serde_json::json!({
                        "code": "RATE_LIMITED",
                        "message": "Rate limit exceeded"
                    }));
                let (req_parts, _) = req.into_parts();
                return Ok(actix_web::dev::ServiceResponse::new(req_parts, response.map_into_boxed_body()));
            }
        }
    }

    let res = next.call(req).await?;
    Ok(res.map_into_boxed_body())
}