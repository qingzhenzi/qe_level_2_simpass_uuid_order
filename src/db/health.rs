use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{interval, Duration};
use crate::db::DbPool;

#[derive(Clone)]
pub struct HealthChecker {
    db_pool: DbPool,
    db_healthy: Arc<AtomicBool>,
    overall_healthy: Arc<AtomicBool>,
}

impl HealthChecker {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            db_healthy: Arc::new(AtomicBool::new(true)),
            overall_healthy: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn db_healthy(&self) -> bool {
        self.db_healthy.load(Ordering::Acquire)
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
        let db_ok = self.check_db().await;
        self.db_healthy.store(db_ok, Ordering::Release);

        let prev = self.overall_healthy.load(Ordering::Acquire);
        self.overall_healthy.store(db_ok, Ordering::Release);

        if prev != db_ok {
            if db_ok {
                log::info!("Service health restored");
            } else {
                log::warn!("Service degraded: db={}", db_ok);
            }
        }
    }

    async fn check_db(&self) -> bool {
        match &self.db_pool {
            DbPool::Postgres(pg) => {
                sqlx::query("SELECT 1").execute(pg).await.is_ok()
            }
            DbPool::Sqlite(sq) => {
                sqlx::query("SELECT 1").execute(sq).await.is_ok()
            }
        }
    }
}
