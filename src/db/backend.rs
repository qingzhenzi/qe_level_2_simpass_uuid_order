use sqlx::PgPool;
use sqlx::SqlitePool;
use crate::config::{Config, DbBackend};
use log::info;

// ── 统一数据库连接池枚举 ──────────────────────────────────────────

#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    /// 根据配置创建连接池
    pub async fn create(cfg: &Config) -> Result<Self, sqlx::Error> {
        match cfg.db_backend {
            DbBackend::Postgres => {
                info!("Connecting to PostgreSQL at {}:{}/{} (user: {})...",
                    cfg.pg_host, cfg.pg_port, cfg.pg_dbname, cfg.pg_user);
                let pool = PgPool::connect(&cfg.pg_conn_string()).await?;
                info!("PostgreSQL connection pool established (search_path = sl_uuid, public)");
                Ok(Self::Postgres(pool))
            }
            DbBackend::Sqlite => {
                // Resolve to absolute path based on project root, add mode=rwc
                let mut resolved_url = if let Some(path) = cfg.sqlite_url.strip_prefix("sqlite:") {
                    let path = path.trim_start_matches("///").trim_start_matches("/");
                    let abs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
                    let path_str = abs_path.display().to_string().replace("\\", "/");
                    format!("sqlite:///{}?mode=rwc", path_str)
                } else {
                    cfg.sqlite_url.clone()
                };

                info!("Connecting to SQLite at {}", resolved_url);

                // Ensure parent directory exists
                if let Some(path) = resolved_url.strip_prefix("sqlite:///") {
                    let path = path.split('?').next().unwrap_or(path);
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        if !parent.as_os_str().is_empty() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                    }
                }

                let pool = SqlitePool::connect(&resolved_url).await?;
                info!("SQLite connection pool established");
                Ok(Self::Sqlite(pool))
            }
        }
    }

    /// 获取是否为 Postgres 后端
    pub fn is_postgres(&self) -> bool {
        matches!(self, Self::Postgres(_))
    }

    /// 获取后端类型
    pub fn backend(&self) -> DbBackend {
        match self {
            Self::Postgres(_) => DbBackend::Postgres,
            Self::Sqlite(_) => DbBackend::Sqlite,
        }
    }
}
