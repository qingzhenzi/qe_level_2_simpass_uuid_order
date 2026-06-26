use crate::db::DbPool;
use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// QPS tracker — works with Redis (distributed) or memory (single-instance).
#[derive(Clone)]
pub struct QpsTracker {
    db_pool: DbPool,
    redis_conn: Option<redis::aio::ConnectionManager>,
    redis_prefix: String,
    aggregation_errors: Arc<AtomicU64>,
    // In-memory fallback counters (used when no Redis)
    memory_total: Arc<parking_lot::Mutex<std::collections::HashMap<String, (i64, i64)>>>,
}

impl QpsTracker {
    pub fn new(
        db_pool: DbPool,
        redis_conn: Option<redis::aio::ConnectionManager>,
        redis_prefix: String,
    ) -> Self {
        Self {
            db_pool,
            redis_conn,
            redis_prefix,
            aggregation_errors: Arc::new(AtomicU64::new(0)),
            memory_total: Arc::new(parking_lot::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn aggregation_errors(&self) -> u64 {
        self.aggregation_errors.load(Ordering::Relaxed)
    }

    // ── record ────────────────────────────────────────────────────────

    pub async fn record_request(&self, api_path: &str, developer_uuid: Option<&str>) {
        let now_sec = Utc::now().timestamp();

        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            use redis::AsyncCommands;

            let total_key = format!("{}:qps:cnt:{}:{}", self.redis_prefix, api_path, now_sec);
            let _: Result<i64, _> = conn.incr(&total_key, 1).await;
            let _: Result<(), _> = conn.expire(&total_key, 7200).await;
            let _: Result<(), _> = conn.sadd(format!("{}:qps:paths", self.redis_prefix), api_path).await;

            if let Some(uuid) = developer_uuid {
                let dev_key = format!("{}:qps:dev:{}:{}:{}", self.redis_prefix, uuid, api_path, now_sec);
                let _: Result<i64, _> = conn.incr(&dev_key, 1).await;
                let _: Result<(), _> = conn.expire(&dev_key, 7200).await;
            }
        } else {
            // Memory fallback — track total per path for current second
            let mut guard = self.memory_total.lock();
            let key = format!("{}:{}", api_path, now_sec);
            let entry = guard.entry(key).or_insert((0, now_sec));
            entry.0 += 1;
        }
    }

    // ── queries ───────────────────────────────────────────────────────

    pub async fn get_current_qps(&self, api_path: &str) -> i64 {
        let now_sec = Utc::now().timestamp();

        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            use redis::AsyncCommands;

            let cur: i64 = conn
                .get(format!("{}:qps:cnt:{}:{}", self.redis_prefix, api_path, now_sec))
                .await
                .unwrap_or(0);
            let prev: i64 = conn
                .get(format!("{}:qps:cnt:{}:{}", self.redis_prefix, api_path, now_sec - 1))
                .await
                .unwrap_or(0);
            cur.max(prev)
        } else {
            // Memory fallback — count from current second
            let guard = self.memory_total.lock();
            let key = format!("{}:{}", api_path, now_sec);
            guard.get(&key).map(|v| v.0).unwrap_or(0)
                + guard.get(&format!("{}:{}", api_path, now_sec - 1)).map(|v| v.0).unwrap_or(0)
        }
    }

    pub async fn get_qps_since(&self, api_path: &str, duration_secs: i64) -> f64 {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;

        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            let total = self.sum_redis_counters(&mut conn, api_path, start_sec, now_sec).await;
            total as f64 / duration_secs as f64
        } else {
            let guard = self.memory_total.lock();
            let mut total = 0i64;
            for s in start_sec..=now_sec {
                let key = format!("{}:{}", api_path, s);
                if let Some(v) = guard.get(&key) {
                    total += v.0;
                }
            }
            if duration_secs > 0 { total as f64 / duration_secs as f64 } else { 0.0 }
        }
    }

    pub async fn get_all_path_counts_since(&self, duration_secs: i64) -> Vec<(String, i64)> {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;

        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            use redis::AsyncCommands;

            let paths: Vec<String> = conn
                .smembers(format!("{}:qps:paths", self.redis_prefix))
                .await
                .unwrap_or_default();

            let mut counts: Vec<(String, i64)> = Vec::with_capacity(paths.len());
            for path in &paths {
                let total = self.sum_redis_counters(&mut conn, path, start_sec, now_sec).await;
                if total > 0 {
                    counts.push((path.clone(), total));
                }
            }

            counts.sort_by(|a, b| b.1.cmp(&a.1));
            counts.truncate(10);
            counts
        } else {
            // Memory fallback — aggregate from in-memory counters
            let guard = self.memory_total.lock();
            let mut path_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
            for s in start_sec..=now_sec {
                for (key, &(count, _)) in guard.iter() {
                    if let Some(ts_str) = key.rsplit_once(':') {
                        if let Ok(ts) = ts_str.1.parse::<i64>() {
                            if ts == s {
                                let path = ts_str.0.to_string();
                                *path_counts.entry(path).or_insert(0) += count;
                            }
                        }
                    }
                }
            }
            let mut result: Vec<(String, i64)> = path_counts.into_iter().collect();
            result.sort_by(|a, b| b.1.cmp(&a.1));
            result.truncate(10);
            result
        }
    }

    pub async fn get_avg_qps_across_paths(&self, duration_secs: i64) -> f64 {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;

        if let Some(ref conn) = self.redis_conn {
            let mut conn = conn.clone();
            use redis::AsyncCommands;

            let paths: Vec<String> = conn
                .smembers(format!("{}:qps:paths", self.redis_prefix))
                .await
                .unwrap_or_default();

            let mut grand_total: i64 = 0;
            for path in &paths {
                grand_total += self.sum_redis_counters(&mut conn, path, start_sec, now_sec).await;
            }
            if duration_secs > 0 { grand_total as f64 / duration_secs as f64 } else { 0.0 }
        } else {
            let counts = self.get_all_path_counts_since(duration_secs).await;
            let total: i64 = counts.iter().map(|(_, c)| c).sum();
            if duration_secs > 0 { total as f64 / duration_secs as f64 } else { 0.0 }
        }
    }

    // ── aggregation to DB ─────────────────────────────────────────────

    pub fn start_aggregation(self) {
        let errors = self.aggregation_errors.clone();
        let db_pool = self.db_pool.clone();
        let redis_conn = self.redis_conn.clone();
        let redis_prefix = self.redis_prefix.clone();
        let memory_total = self.memory_total.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;
                if let Err(e) = aggregate(&db_pool, &redis_conn, &redis_prefix, &memory_total).await {
                    log::error!("QPS aggregation error: {}", e);
                    errors.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }

    // ── internal helpers ──────────────────────────────────────────────

    async fn sum_redis_counters(
        &self,
        conn: &mut redis::aio::ConnectionManager,
        path: &str,
        start_sec: i64,
        end_sec: i64,
    ) -> i64 {
        let prefix = &self.redis_prefix;
        let keys: Vec<String> = (start_sec..=end_sec)
            .map(|s| format!("{}:qps:cnt:{}:{}", prefix, path, s))
            .collect();

        if keys.is_empty() {
            return 0;
        }

        let result: Vec<Option<i64>> = redis::cmd("MGET")
            .arg(&keys)
            .query_async(conn)
            .await
            .unwrap_or_default();
        result.into_iter().flatten().sum()
    }
}

// ── 聚合函数 ──────────────────────────────────────────────────────

async fn aggregate(
    db_pool: &DbPool,
    redis_conn: &Option<redis::aio::ConnectionManager>,
    redis_prefix: &str,
    memory_total: &Arc<parking_lot::Mutex<std::collections::HashMap<String, (i64, i64)>>>,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let one_min_ago = now - chrono::Duration::seconds(60);
    let end_sec = now.timestamp();
    let start_sec = one_min_ago.timestamp();

    let mut entries: Vec<(String, i32)> = Vec::new();

    if let Some(ref conn) = redis_conn {
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        let paths: Vec<String> = conn
            .smembers(format!("{}:qps:paths", redis_prefix))
            .await
            .unwrap_or_default();

        for path in &paths {
            let keys: Vec<String> = (start_sec..=end_sec)
                .map(|s| format!("{}:qps:cnt:{}:{}", redis_prefix, path, s))
                .collect();

            if keys.is_empty() { continue; }

            let result: Vec<Option<i64>> = redis::cmd("MGET")
                .arg(&keys)
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            let total: i64 = result.into_iter().flatten().sum();
            if total > 0 {
                entries.push((path.clone(), total as i32));
            }
        }
    } else {
        // Memory fallback — collect entries from the last minute
        let guard = memory_total.lock();
        let mut path_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for s in start_sec..=end_sec {
            for (key, &(count, ts)) in guard.iter() {
                if ts == s {
                    if let Some((path, _)) = key.rsplit_once(':') {
                        *path_counts.entry(path.to_string()).or_insert(0) += count;
                    }
                }
            }
        }
        for (path, count) in path_counts {
            if count > 0 {
                entries.push((path, count as i32));
            }
        }
    }

    if entries.is_empty() {
        return Ok(());
    }

    match db_pool {
        DbPool::Postgres(pg) => {
            let mut builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
                "INSERT INTO qps_records (api_path, total_qps, recorded_at) "
            );
            builder.push_values(entries.iter(), |mut b, (api_path, count)| {
                b.push_bind(api_path).push_bind(*count).push_bind(now.to_rfc3339());
            });
            builder.build().execute(pg).await?;
        }
        DbPool::Sqlite(sq) => {
            let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "INSERT INTO qps_records (api_path, total_qps, recorded_at) "
            );
            builder.push_values(entries.iter(), |mut b, (api_path, count)| {
                b.push_bind(api_path).push_bind(*count).push_bind(now.to_rfc3339());
            });
            builder.build().execute(sq).await?;
        }
    }

    Ok(())
}
