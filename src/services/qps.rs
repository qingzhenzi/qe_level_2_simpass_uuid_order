use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use dashmap::DashMap;
use chrono::{Utc, DateTime};
use tokio::time::{interval, Duration};
use sqlx::PgPool;

const MAX_ENTRIES_PER_KEY: usize = 300;

#[derive(Clone)]
pub struct QpsTracker {
    sliding_windows: Arc<DashMap<String, VecDeque<DateTime<Utc>>>>,
    pg_pool: PgPool,
}

impl QpsTracker {
    pub fn new(pg_pool: PgPool) -> Self {
        Self {
            sliding_windows: Arc::new(DashMap::new()),
            pg_pool,
        }
    }

    #[allow(dead_code)]
    pub fn record_request(&self, api_path: &str, developer_uuid: Option<&str>) {
        let now = Utc::now();

        let key = format!("total:{}", api_path);
        let mut entry = self.sliding_windows.entry(key).or_insert_with(VecDeque::new);
        if entry.len() >= MAX_ENTRIES_PER_KEY {
            entry.pop_front();
        }
        entry.push_back(now);

        if let Some(uuid) = developer_uuid {
            let dev_key = format!("dev:{}:{}", uuid, api_path);
            let mut dev_entry = self.sliding_windows.entry(dev_key).or_insert_with(VecDeque::new);
            if dev_entry.len() >= MAX_ENTRIES_PER_KEY {
                dev_entry.pop_front();
            }
            dev_entry.push_back(now);
        }
    }

    pub fn get_current_qps(&self, api_path: &str) -> i64 {
        let now = Utc::now();
        let one_sec_ago = now - chrono::Duration::seconds(1);
        let key = format!("total:{}", api_path);

        self.sliding_windows
            .get(&key)
            .map(|times| {
                times.iter().filter(|t| **t > one_sec_ago).count() as i64
            })
            .unwrap_or(0)
    }

    pub fn get_qps_since(&self, api_path: &str, duration_secs: i64) -> f64 {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(duration_secs);
        let key = format!("total:{}", api_path);

        let count = self.sliding_windows
            .get(&key)
            .map(|times| {
                times.iter().filter(|t| **t > cutoff).count() as f64
            })
            .unwrap_or(0.0);

        count / duration_secs as f64
    }

    pub fn get_all_path_counts_since(&self, duration_secs: i64) -> Vec<(String, i64)> {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(duration_secs);
        let mut counts: HashMap<String, i64> = HashMap::new();

        for entry in self.sliding_windows.iter() {
            if entry.key().starts_with("total:") {
                let path = entry.key().replace("total:", "");
                let count = entry.value().iter().filter(|t| **t > cutoff).count() as i64;
                if count > 0 {
                    counts.insert(path, count);
                }
            }
        }

        let mut pairs: Vec<(String, i64)> = counts.into_iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(10);
        pairs
    }

    pub fn get_avg_qps_across_paths(&self, duration_secs: i64) -> f64 {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(duration_secs);
        let mut total = 0.0;

        for entry in self.sliding_windows.iter() {
            if entry.key().starts_with("total:") {
                let count = entry.value().iter().filter(|t| **t > cutoff).count() as f64;
                total += count;
            }
        }

        total / duration_secs as f64
    }

    pub fn cleanup_old_entries(&self) {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::hours(2);
        self.sliding_windows.retain(|_, times| {
            times.retain(|t| *t > cutoff);
            !times.is_empty()
        });
    }

    pub fn start_cleanup(self) {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(300));
            loop {
                ticker.tick().await;
                self.cleanup_old_entries();
            }
        });
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

        let entries: Vec<(String, i32)> = self
            .sliding_windows
            .iter()
            .filter(|e| e.key().starts_with("total:"))
            .map(|e| {
                let count = e.value().iter().filter(|t| **t > one_min_ago).count() as i32;
                (e.key().replace("total:", ""), count)
            })
            .collect();

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
