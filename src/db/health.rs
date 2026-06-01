use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{interval, Duration};
use sqlx::PgPool;
use redis::aio::ConnectionManager;

#[derive(Clone)]
pub struct HealthChecker {
    pg_pool: PgPool,
    redis_conn: Option<ConnectionManager>,
    pg_healthy: Arc<AtomicBool>,
    redis_healthy: Arc<AtomicBool>,
    overall_healthy: Arc<AtomicBool>,
}

impl HealthChecker {
    pub fn new(pg_pool: PgPool, redis_conn: Option<ConnectionManager>) -> Self {
        let has_redis = redis_conn.is_some();
        Self {
            pg_pool,
            redis_conn,
            pg_healthy: Arc::new(AtomicBool::new(true)),
            redis_healthy: Arc::new(AtomicBool::new(has_redis)),
            overall_healthy: Arc::new(AtomicBool::new(true)),
        }
    }

    #[allow(dead_code)]
    pub fn pg_healthy(&self) -> bool {
        self.pg_healthy.load(Ordering::Acquire)
    }

    #[allow(dead_code)]
    pub fn redis_healthy(&self) -> bool {
        self.redis_healthy.load(Ordering::Acquire)
    }

    pub fn is_healthy(&self) -> bool {
        self.overall_healthy.load(Ordering::Acquire)
    }

    pub fn start_health_check(self) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(10));
            loop {
                ticker.tick().await;
                self.check_all().await;
            }
        });
    }

    async fn check_all(&self) {
        let pg_ok = self.check_pg().await;
        self.pg_healthy.store(pg_ok, Ordering::Release);

        let redis_ok = if self.redis_conn.is_some() {
            self.check_redis().await
        } else {
            false
        };
        self.redis_healthy.store(redis_ok, Ordering::Release);

        let overall = pg_ok;
        let prev = self.overall_healthy.load(Ordering::Acquire);

        self.overall_healthy.store(overall, Ordering::Release);

        if prev != overall {
            if overall {
                log::info!("Service health restored");
            } else {
                log::warn!(
                    "Service degraded: pg={}, redis={}",
                    pg_ok, redis_ok
                );
            }
        } else if !redis_ok && self.redis_conn.is_some() {
            log::warn!("Redis unavailable, running in degraded mode");
        }
    }

    async fn check_pg(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.pg_pool)
            .await
            .is_ok()
    }

    async fn check_redis(&self) -> bool {
        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            let pong: String = redis::cmd("PING")
                .query_async(&mut conn)
                .await
                .unwrap_or_default();
            pong.to_uppercase() == "PONG"
        } else {
            false
        }
    }
}
