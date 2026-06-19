use chrono::Utc;
use sqlx::PgPool;
use redis::aio::ConnectionManager;
use tokio::time::{interval, Duration};

/// Redis-based QPS tracker — shared across instances via per-second counters.
///
/// Key schema:
///   {prefix}:qps:paths              — SET of active path names
///   {prefix}:qps:cnt:{path}:{epoch} — INCR counter for one second+path
///   {prefix}:qps:dev:{uuid}:{path}:{epoch} — per-developer counter
///
/// All counters get a 2-hour TTL so they self-clean.
#[derive(Clone)]
pub struct QpsTracker {
    pg_pool: PgPool,
    redis_conn: Option<ConnectionManager>,
    redis_prefix: String,
}

impl QpsTracker {
    pub fn new(
        pg_pool: PgPool,
        redis_conn: Option<ConnectionManager>,
        redis_prefix: String,
    ) -> Self {
        Self {
            pg_pool,
            redis_conn,
            redis_prefix,
        }
    }

    // ── record ────────────────────────────────────────────────────────

    pub async fn record_request(&self, api_path: &str, developer_uuid: Option<&str>) {
        let now_sec = Utc::now().timestamp();
        let prefix = &self.redis_prefix;

        let Some(ref conn) = self.redis_conn else { return };
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        // total per path
        let total_key = format!("{}:qps:cnt:{}:{}", prefix, api_path, now_sec);
        let _: Result<i64, _> = conn.incr(&total_key, 1).await;
        let _: Result<(), _> = conn.expire(&total_key, 7200).await;
        let _: Result<(), _> = conn.sadd(format!("{}:qps:paths", prefix), api_path).await;

        // per-developer
        if let Some(uuid) = developer_uuid {
            let dev_key = format!("{}:qps:dev:{}:{}:{}", prefix, uuid, api_path, now_sec);
            let _: Result<i64, _> = conn.incr(&dev_key, 1).await;
            let _: Result<(), _> = conn.expire(&dev_key, 7200).await;
        }
    }

    // ── queries ───────────────────────────────────────────────────────

    pub async fn get_current_qps(&self, api_path: &str) -> i64 {
        let now_sec = Utc::now().timestamp();
        let prefix = &self.redis_prefix;

        let Some(ref conn) = self.redis_conn else { return 0 };
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        let cur: i64 = conn
            .get(format!("{}:qps:cnt:{}:{}", prefix, api_path, now_sec))
            .await
            .unwrap_or(0);
        let prev: i64 = conn
            .get(format!("{}:qps:cnt:{}:{}", prefix, api_path, now_sec - 1))
            .await
            .unwrap_or(0);
        cur.max(prev)
    }

    pub async fn get_qps_since(&self, api_path: &str, duration_secs: i64) -> f64 {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;
        let prefix = &self.redis_prefix;

        if self.redis_conn.is_none() {
            return 0.0;
        }
        let total = self
            .sum_counters(prefix, api_path, start_sec, now_sec)
            .await;
        total as f64 / duration_secs as f64
    }

    pub async fn get_all_path_counts_since(&self, duration_secs: i64) -> Vec<(String, i64)> {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;
        let prefix = &self.redis_prefix;

        let Some(ref conn) = self.redis_conn else { return vec![] };
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        let paths: Vec<String> = conn
            .smembers(format!("{}:qps:paths", prefix))
            .await
            .unwrap_or_default();

        let mut counts: Vec<(String, i64)> = Vec::with_capacity(paths.len());
        for path in &paths {
            let total = self.sum_counters(prefix, path, start_sec, now_sec).await;
            if total > 0 {
                counts.push((path.clone(), total));
            }
        }

        counts.sort_by(|a, b| b.1.cmp(&a.1));
        counts.truncate(10);
        counts
    }

    pub async fn get_avg_qps_across_paths(&self, duration_secs: i64) -> f64 {
        let now_sec = Utc::now().timestamp();
        let start_sec = now_sec - duration_secs;
        let prefix = &self.redis_prefix;

        let Some(ref conn) = self.redis_conn else { return 0.0 };
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        let paths: Vec<String> = conn
            .smembers(format!("{}:qps:paths", prefix))
            .await
            .unwrap_or_default();

        let mut grand_total: i64 = 0;
        for path in &paths {
            grand_total += self.sum_counters(prefix, path, start_sec, now_sec).await;
        }
        grand_total as f64 / duration_secs as f64
    }

    // ── aggregation to DB ─────────────────────────────────────────────

    async fn sum_counters(&self, prefix: &str, path: &str, start_sec: i64, end_sec: i64) -> i64 {
        let Some(ref conn) = self.redis_conn else { return 0 };
        let mut conn = conn.clone();

        let keys: Vec<String> = (start_sec..=end_sec)
            .map(|s| format!("{}:qps:cnt:{}:{}", prefix, path, s))
            .collect();

        if keys.is_empty() {
            return 0;
        }

        let result: Vec<Option<i64>> = redis::cmd("MGET")
            .arg(&keys)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();
        result.into_iter().flatten().sum()
    }

    pub fn start_aggregation(self) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));
            loop {
                ticker.tick().await;
                if let Err(e) = self.aggregate_to_db().await {
                    log::error!("QPS aggregation error: {}", e);
                }
            }
        });
    }

    async fn aggregate_to_db(&self) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        let one_min_ago = now - chrono::Duration::seconds(60);
        let end_sec = now.timestamp();
        let start_sec = one_min_ago.timestamp();
        let prefix = &self.redis_prefix;

        let Some(ref conn) = self.redis_conn else { return Ok(()) };
        let mut conn = conn.clone();
        use redis::AsyncCommands;

        let paths: Vec<String> = conn
            .smembers(format!("{}:qps:paths", prefix))
            .await
            .unwrap_or_default();

        let mut entries = Vec::with_capacity(paths.len());
        for path in &paths {
            let total = self.sum_counters(prefix, path, start_sec, end_sec).await;
            if total > 0 {
                entries.push((path.clone(), total as i32));
            }
        }

        if entries.is_empty() {
            return Ok(());
        }

        let mut builder = sqlx::query_builder::QueryBuilder::new(
            "INSERT INTO qps_records (api_path, total_qps, recorded_at) ",
        );
        builder.push_values(entries.iter(), |mut b, (api_path, count)| {
            b.push_bind(api_path).push_bind(*count).push_bind(now);
        });
        builder.build().execute(&self.pg_pool).await?;
        Ok(())
    }
}
