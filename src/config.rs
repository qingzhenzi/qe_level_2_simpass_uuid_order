use std::env;

// ── 后端选择枚举 ──────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum DbBackend {
    Postgres,
    Sqlite,
}

impl DbBackend {
    pub fn from_env() -> Self {
        match env::var("DB_BACKEND").unwrap_or_default().to_lowercase().as_str() {
            "sqlite" | "sqlite3" => Self::Sqlite,
            _ => Self::Postgres,
        }
    }

    pub fn is_postgres(&self) -> bool {
        matches!(self, Self::Postgres)
    }

    pub fn is_sqlite(&self) -> bool {
        matches!(self, Self::Sqlite)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CacheBackend {
    Redis,
    Memory,
}

impl CacheBackend {
    pub fn from_env() -> Self {
        match env::var("CACHE_BACKEND").unwrap_or_default().to_lowercase().as_str() {
            "memory" | "mem" | "inmemory" => Self::Memory,
            _ => Self::Redis,
        }
    }

    pub fn is_redis(&self) -> bool {
        matches!(self, Self::Redis)
    }

    pub fn is_memory(&self) -> bool {
        matches!(self, Self::Memory)
    }
}

// ── 配置结构 ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub db_backend: DbBackend,
    pub cache_backend: CacheBackend,

    // Postgres
    pub pg_host: String,
    pub pg_port: u16,
    pub pg_user: String,
    pub pg_password: String,
    pub pg_dbname: String,

    // SQLite
    pub sqlite_url: String,

    // Redis
    pub redis_url: String,
    pub redis_prefix: String,

    // Deduction
    pub deduction_timeout_secs: u64,
    pub deduction_allowed_useragents: Vec<String>,
    pub deduction_api_token: Option<String>,

    // Admin
    pub admin_api_token: Option<String>,

    // Log
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .expect("SERVER_PORT must be a number"),
            db_backend: DbBackend::from_env(),
            cache_backend: CacheBackend::from_env(),
            pg_host: env::var("PG_HOST").unwrap_or_else(|_| "localhost".into()),
            pg_port: env::var("PG_PORT")
                .unwrap_or_else(|_| "5432".into())
                .parse()
                .expect("PG_PORT must be a number"),
            pg_user: env::var("PG_USER").unwrap_or_else(|_| "postgres".into()),
            pg_password: env::var("PG_PASSWORD").unwrap_or_else(|_| "".into()),
            pg_dbname: env::var("PG_DBNAME").unwrap_or_else(|_| "postgres".into()),
            sqlite_url: env::var("SQLITE_URL").unwrap_or_else(|_| "sqlite:data/app.db".into()),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()),
            redis_prefix: env::var("REDIS_PREFIX").unwrap_or_else(|_| "sl:uuid".into()),
            deduction_timeout_secs: env::var("DEDUCTION_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .expect("DEDUCTION_TIMEOUT_SECS must be a number"),
            deduction_allowed_useragents: env::var("DEDUCTION_ALLOWED_USERAGENTS")
                .unwrap_or_else(|_| "".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            deduction_api_token: env::var("DEDUCTION_API_TOKEN").ok(),
            admin_api_token: env::var("ADMIN_API_TOKEN").ok(),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        }
    }

    pub fn pg_conn_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.pg_user, self.pg_password, self.pg_host, self.pg_port, self.pg_dbname
        )
    }
}
