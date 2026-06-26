use std::time::{Duration, Instant};
use uuid::Uuid;
use dashmap::DashMap;
use crate::models::DeductionTransaction;

// ── 内存缓存条目（带 TTL）─────────────────────────────────────────

#[derive(Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

// ── 内存缓存 ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct MemoryCache {
    prefix: String,
    // developer deduction data
    deduction_data: DashMap<Uuid, CacheEntry<(i32, i32)>>,
    // pending counts per developer
    pending: DashMap<Uuid, CacheEntry<i64>>,
    // transactions
    transactions: DashMap<Uuid, CacheEntry<DeductionTransaction>>,
    // processed flag (idempotency)
    processed: DashMap<Uuid, ()>,
}

impl MemoryCache {
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            deduction_data: DashMap::new(),
            pending: DashMap::new(),
            transactions: DashMap::new(),
            processed: DashMap::new(),
        }
    }

    // ── 扣量数据 ──────────────────────────────────────────────────

    pub async fn get_deduction_data(&self, dev_uuid: Uuid) -> Option<(i32, i32)> {
        self.cleanup_expired(&self.deduction_data, dev_uuid);
        self.deduction_data.get(&dev_uuid).map(|e| e.value)
    }

    pub async fn set_deduction_data(&self, dev_uuid: Uuid, available: i32, limit: i32) {
        self.deduction_data.insert(dev_uuid, CacheEntry::new((available, limit), Duration::from_secs(7200)));
        log::debug!("[MemoryCache] set_deduction_data: dev={} avail={} limit={}", dev_uuid, available, limit);
    }

    // ── Pending 计数 ──────────────────────────────────────────────

    pub async fn get_pending_count(&self, dev_uuid: Uuid) -> i64 {
        self.cleanup_expired(&self.pending, dev_uuid);
        self.pending.get(&dev_uuid).map(|e| e.value).unwrap_or(0)
    }

    pub async fn add_pending(&self, dev_uuid: Uuid, amount: i64, ttl_secs: u64) {
        let mut entry = self.pending.entry(dev_uuid).or_insert_with(|| CacheEntry::new(0, Duration::from_secs(ttl_secs)));
        entry.value_mut().value += amount;
        entry.value_mut().expires_at = Instant::now() + Duration::from_secs(ttl_secs);
    }

    pub async fn remove_pending(&self, dev_uuid: Uuid, amount: i64) {
        if let Some(mut entry) = self.pending.get_mut(&dev_uuid) {
            entry.value_mut().value = (entry.value().value - amount).max(0);
        }
    }

    pub async fn incr_pending_count(&self, dev_uuid: Uuid) {
        let mut entry = self.pending.entry(dev_uuid).or_insert_with(|| CacheEntry::new(0, Duration::from_secs(7200)));
        entry.value_mut().value += 1;
    }

    pub async fn decr_pending_count(&self, dev_uuid: Uuid) {
        if let Some(mut entry) = self.pending.get_mut(&dev_uuid) {
            entry.value_mut().value = (entry.value().value - 1).max(0);
        }
    }

    // ── 扣量/退款 ─────────────────────────────────────────────────

    pub async fn deduct(&self, dev_uuid: Uuid, amount: i32) -> Result<(bool, i32), crate::errors::AppError> {
        let (current_available, current_limit) = match self.get_deduction_data(dev_uuid).await {
            Some(v) => v,
            None => return Ok((false, 0)),
        };

        if current_available < amount {
            return Ok((false, current_available));
        }

        let new_val = current_available - amount;
        self.deduction_data.insert(dev_uuid, CacheEntry::new((new_val, current_limit), Duration::from_secs(7200)));
        Ok((true, new_val))
    }

    pub async fn refund(&self, dev_uuid: Uuid, amount: i32) {
        if let Some((current_available, current_limit)) = self.get_deduction_data(dev_uuid).await {
            let new_val = (current_available + amount).min(current_limit);
            self.deduction_data.insert(dev_uuid, CacheEntry::new((new_val, current_limit), Duration::from_secs(7200)));
        }
    }

    // ── 事务存储 ──────────────────────────────────────────────────

    pub async fn set_transaction(&self, tx: &DeductionTransaction, ttl_secs: u64) {
        self.transactions.insert(tx.transaction_token, CacheEntry::new(tx.clone(), Duration::from_secs(ttl_secs)));
    }

    pub async fn del_transaction(&self, tx_token: Uuid) {
        self.transactions.remove(&tx_token);
    }

    // ── 幂等性（已处理标记） ──────────────────────────────────────

    pub async fn try_claim_processed(&self, tx_token: Uuid) -> bool {
        match self.processed.entry(tx_token) {
            dashmap::Entry::Vacant(entry) => {
                entry.insert(());
                true
            }
            dashmap::Entry::Occupied(_) => false,
        }
    }

    pub async fn del_processed(&self, tx_token: Uuid) {
        self.processed.remove(&tx_token);
    }

    // ── 初始数据加载 ─────────────────────────────────────────────

    pub async fn load_initial_data(&self, dev_uuid: Uuid, available: i32, limit: i32) {
        if !self.deduction_data.contains_key(&dev_uuid) {
            self.deduction_data.insert(dev_uuid, CacheEntry::new((available, limit), Duration::from_secs(7200)));
            log::warn!("[MemoryCache] Loaded initial data for dev: available={}, limit={}", available, limit);
        }
    }

    // ── 内部：清理过期条目 ────────────────────────────────────────

    fn cleanup_expired<K, V>(&self, map: &DashMap<K, CacheEntry<V>>, key: K)
    where
        K: std::hash::Hash + Eq + Clone,
    {
        if let Some(entry) = map.get(&key) {
            if entry.is_expired() {
                map.remove(&key);
            }
        }
    }
}
