use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub server_host: String,
    pub server_port: u16,
    pub pg_host: String,
    pub pg_port: u16,
    pub pg_user: String,
    pub pg_password: String,
    pub pg_dbname: String,
    pub redis_url: String,
    pub redis_prefix: String,
    pub deduction_timeout_secs: u64,
    pub log_level: String,
    #[allow(dead_code)]
    pub deduction_allowed_useragents: Vec<String>,
    #[allow(dead_code)]
    pub deduction_api_token: Option<String>,
    #[allow(dead_code)]
    pub admin_api_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".into())
                .parse()
                .expect("SERVER_PORT must be a number"),
            pg_host: env::var("PG_HOST").unwrap_or_else(|_| "localhost".into()),
            pg_port: env::var("PG_PORT")
                .unwrap_or_else(|_| "5432".into())
                .parse()
                .expect("PG_PORT must be a number"),
            pg_user: env::var("PG_USER").unwrap_or_else(|_| "postgres".into()),
            pg_password: env::var("PG_PASSWORD").unwrap_or_else(|_| "".into()),
            pg_dbname: env::var("PG_DBNAME").unwrap_or_else(|_| "postgres".into()),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()),
            redis_prefix: env::var("REDIS_PREFIX")
                .unwrap_or_else(|_| "app".into()),
            deduction_timeout_secs: env::var("DEDUCTION_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .expect("DEDUCTION_TIMEOUT_SECS must be a number"),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
            deduction_allowed_useragents: env::var("DEDUCTION_ALLOWED_USERAGENTS")
                .unwrap_or_else(|_| "".into())
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            deduction_api_token: env::var("DEDUCTION_API_TOKEN").ok(),
            admin_api_token: env::var("ADMIN_API_TOKEN").ok(),
        }
    }

    pub fn pg_conn_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.pg_user, self.pg_password, self.pg_host, self.pg_port, self.pg_dbname
        )
    }
}