use sqlx::postgres::{PgPool, PgPoolOptions};
use crate::config::Config;

pub async fn create_pool(config: &Config) -> Result<PgPool, sqlx::Error> {
    // 隐藏连接字符串中的敏感信息（用户名和密码）
    log::info!("Connecting to PostgreSQL at {}:{} (user: {})...", config.pg_host, config.pg_port, config.pg_user);

    let pool = PgPoolOptions::new()
        .max_connections(1000)
        .min_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(60))
        .idle_timeout(std::time::Duration::from_secs(120))
        .max_lifetime(std::time::Duration::from_secs(3600))
        .after_connect(|conn, _meta| Box::pin(async move {
            // 设置 schema 搜索路径，使得无前缀的表名默认指向 sl_uuid
            sqlx::query("SET search_path TO sl_uuid, public")
                .execute(conn)
                .await?;
            Ok(())
        }))
        .connect(&config.pg_conn_string())
        .await?;

    log::info!("PostgreSQL connection pool established (search_path = sl_uuid, public)");
    Ok(pool)
}
