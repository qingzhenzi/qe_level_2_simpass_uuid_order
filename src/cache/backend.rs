use crate::cache::memory::MemoryCache;
use crate::cache::redis::RedisCache;
use redis::aio::ConnectionManager;
use uuid::Uuid;
use crate::models::DeductionTransaction;

// ── 统一缓存后端 ──────────────────────────────────────────────────

#[derive(Clone)]
pub enum CacheBackend {
    Redis(RedisCache),
    Memory(MemoryCache),
}

impl CacheBackend {
    pub fn redis(conn: Option<ConnectionManager>, prefix: String) -> Self {
        Self::Redis(RedisCache::new(conn, prefix))
    }

    pub fn memory(prefix: String) -> Self {
        Self::Memory(MemoryCache::new(prefix))
    }

    pub fn from_config(cfg_backend: &crate::config::CacheBackend, conn: Option<ConnectionManager>, prefix: String) -> Self {
        match cfg_backend {
            crate::config::CacheBackend::Redis => Self::Redis(RedisCache::new(conn, prefix)),
            crate::config::CacheBackend::Memory => Self::Memory(MemoryCache::new(prefix)),
        }
    }

    // ── 扣量数据 ──────────────────────────────────────────────────

    pub async fn get_deduction_data(&mut self, dev_uuid: Uuid) -> Option<(i32, i32)> {
        match self {
            Self::Redis(c) => c.get_deduction_data(dev_uuid).await,
            Self::Memory(c) => c.get_deduction_data(dev_uuid).await,
        }
    }

    pub async fn set_deduction_data(&mut self, dev_uuid: Uuid, available: i32, limit: i32) {
        match self {
            Self::Redis(c) => c.set_deduction_data(dev_uuid, available, limit).await,
            Self::Memory(c) => c.set_deduction_data(dev_uuid, available, limit).await,
        }
    }

    pub async fn get_pending_count(&mut self, dev_uuid: Uuid) -> i64 {
        match self {
            Self::Redis(c) => c.get_pending_count(dev_uuid).await,
            Self::Memory(c) => c.get_pending_count(dev_uuid).await,
        }
    }

    pub async fn add_pending(&mut self, dev_uuid: Uuid, amount: i64, ttl_secs: u64) {
        match self {
            Self::Redis(c) => c.add_pending(dev_uuid, amount, ttl_secs).await,
            Self::Memory(c) => c.add_pending(dev_uuid, amount, ttl_secs).await,
        }
    }

    pub async fn remove_pending(&mut self, dev_uuid: Uuid, amount: i64) {
        match self {
            Self::Redis(c) => c.remove_pending(dev_uuid, amount).await,
            Self::Memory(c) => c.remove_pending(dev_uuid, amount).await,
        }
    }

    pub async fn incr_pending_count(&mut self, dev_uuid: Uuid) {
        match self {
            Self::Redis(c) => c.incr_pending_count(dev_uuid).await,
            Self::Memory(c) => c.incr_pending_count(dev_uuid).await,
        }
    }

    pub async fn decr_pending_count(&mut self, dev_uuid: Uuid) {
        match self {
            Self::Redis(c) => c.decr_pending_count(dev_uuid).await,
            Self::Memory(c) => c.decr_pending_count(dev_uuid).await,
        }
    }

    pub async fn deduct(&mut self, dev_uuid: Uuid, amount: i32) -> Result<(bool, i32), crate::errors::AppError> {
        match self {
            Self::Redis(c) => c.deduct(dev_uuid, amount).await,
            Self::Memory(c) => c.deduct(dev_uuid, amount).await,
        }
    }

    pub async fn refund(&mut self, dev_uuid: Uuid, amount: i32) {
        match self {
            Self::Redis(c) => c.refund(dev_uuid, amount).await,
            Self::Memory(c) => c.refund(dev_uuid, amount).await,
        }
    }

    pub async fn set_transaction(&mut self, tx: &DeductionTransaction, ttl_secs: u64) {
        match self {
            Self::Redis(c) => c.set_transaction(tx, ttl_secs).await,
            Self::Memory(c) => c.set_transaction(tx, ttl_secs).await,
        }
    }

    pub async fn del_transaction(&mut self, tx_token: Uuid) {
        match self {
            Self::Redis(c) => c.del_transaction(tx_token).await,
            Self::Memory(c) => c.del_transaction(tx_token).await,
        }
    }

    pub async fn try_claim_processed(&mut self, tx_token: Uuid) -> bool {
        match self {
            Self::Redis(c) => c.try_claim_processed(tx_token).await,
            Self::Memory(c) => c.try_claim_processed(tx_token).await,
        }
    }

    pub async fn del_processed(&mut self, tx_token: Uuid) {
        match self {
            Self::Redis(c) => c.del_processed(tx_token).await,
            Self::Memory(c) => c.del_processed(tx_token).await,
        }
    }

    pub async fn load_initial_data(&mut self, dev_uuid: Uuid, available: i32, limit: i32) {
        match self {
            Self::Redis(c) => c.load_initial_data(dev_uuid, available, limit).await,
            Self::Memory(c) => c.load_initial_data(dev_uuid, available, limit).await,
        }
    }

    /// 是否为 Redis 后端（用于需要分布式特性的代码路径）
    pub fn is_redis(&self) -> bool {
        matches!(self, Self::Redis(_))
    }

    /// 获取底层 Redis 连接（仅 Redis 模式可用）
    pub fn as_redis(&self) -> Option<&RedisCache> {
        match self {
            Self::Redis(c) => Some(c),
            Self::Memory(_) => None,
        }
    }

    pub fn as_redis_mut(&mut self) -> Option<&mut RedisCache> {
        match self {
            Self::Redis(c) => Some(c),
            Self::Memory(_) => None,
        }
    }
}
