use crate::config::DbBackend;
use crate::db::DbPool;
use log::{info, warn, error};
use sha2::{Sha256, Digest};
use std::time::Instant;
use std::path::Path;

// ── 迁移配置 ──────────────────────────────────────────────────────

const PG_MIGRATIONS_DIR: &str = "migrations/init";
const PG_CURRENT_VERSION: i64 = 1;
const SQLITE_MIGRATIONS_DIR: &str = "migrations/sqlite";
const SQLITE_CURRENT_VERSION: i64 = 1;

/// 运行数据库迁移（自动检测后端）
pub async fn run_migrations(pool: &DbPool, db_backend: &DbBackend) {
    match db_backend {
        DbBackend::Postgres => run_postgres_migrations(pool).await,
        DbBackend::Sqlite => run_sqlite_migrations(pool).await,
    }
}

// ── Postgres 迁移逻辑 ─────────────────────────────────────────────

async fn run_postgres_migrations(pool: &DbPool) {
    // 确保 schema 存在
    if let DbPool::Postgres(pg) = pool {
        let _ = sqlx::raw_sql("CREATE SCHEMA IF NOT EXISTS sl_uuid").execute(pg).await;
    }

    // 检查迁移表是否存在
    let exists = match pool {
        DbPool::Postgres(pg) => {
            sqlx::query_scalar(
                "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'sl_uuid' AND table_name = '__migrations')"
            )
            .fetch_one(pg)
            .await
            .unwrap_or(false)
        }
        DbPool::Sqlite(_) => false,
    };

    if !exists {
        if let DbPool::Postgres(pg) = pool {
            let create_sql = r#"
                CREATE TABLE IF NOT EXISTS sl_uuid.__migrations (
                    id BIGSERIAL PRIMARY KEY,
                    version BIGINT NOT NULL UNIQUE,
                    name VARCHAR(255) NOT NULL,
                    description TEXT,
                    checksum VARCHAR(64),
                    applied_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
                    applied_by VARCHAR(255) DEFAULT CURRENT_USER,
                    execution_time_ms BIGINT
                );
                CREATE UNIQUE INDEX IF NOT EXISTS idx_migrations_version ON sl_uuid.__migrations(version);
                CREATE INDEX IF NOT EXISTS idx_migrations_applied_at ON sl_uuid.__migrations(applied_at DESC);
            "#;
            let _ = sqlx::raw_sql(create_sql).execute(pg).await;
            info!("[MIGRATION] Postgres migration table created");
        }
    }

    let last_version = match pool {
        DbPool::Postgres(pg) => {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT COALESCE(MAX(version), 0) FROM sl_uuid.__migrations"
            )
            .fetch_one(pg)
            .await
            .ok()
            .flatten()
            .unwrap_or(0)
        }
        DbPool::Sqlite(_) => 0,
    };

    if last_version >= PG_CURRENT_VERSION {
        info!("[MIGRATION] No new migrations. Current version: {}", last_version);
        return;
    }

    info!("[MIGRATION] Running Postgres migration v{} -> v{}", last_version, PG_CURRENT_VERSION);

    let sql_files = collect_sql_files(PG_MIGRATIONS_DIR);
    if sql_files.is_empty() {
        info!("[MIGRATION] No SQL files found in {}", PG_MIGRATIONS_DIR);
        return;
    }

    for (i, file) in sql_files.iter().enumerate() {
        let relative = file.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join(PG_MIGRATIONS_DIR))
            .unwrap_or(file);
        info!("  {}. {}", i + 1, relative.to_string_lossy());
    }

    let combined_sql = combine_sql_files(&sql_files);
    let checksum = calculate_checksum(&combined_sql);

    let start = Instant::now();
    if let DbPool::Postgres(pg) = pool {
        match execute_pg_migration(pg, &combined_sql).await {
            Ok(_) => {
                let elapsed = start.elapsed().as_millis();
                let _ = record_pg_migration(pg, PG_CURRENT_VERSION, checksum, elapsed).await;
                info!("[MIGRATION] Postgres migration completed in {}ms", elapsed);
            }
            Err(e) => {
                error!("[MIGRATION] Postgres migration failed: {}", e);
            }
        }
    }
}

async fn execute_pg_migration(pg: &sqlx::PgPool, sql: &str) -> Result<(), sqlx::Error> {
    for stmt in split_sql_statements(sql) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        sqlx::raw_sql(trimmed).execute(pg).await?;
    }
    Ok(())
}

async fn record_pg_migration(pg: &sqlx::PgPool, version: i64, checksum: String, elapsed_ms: u128) -> Result<(), sqlx::Error> {
    let _ = sqlx::raw_sql(
        "ALTER TABLE sl_uuid.__migrations DROP CONSTRAINT IF EXISTS __migrations_name_key"
    ).execute(pg).await;

    sqlx::query(r#"
        INSERT INTO sl_uuid.__migrations (version, name, description, checksum, execution_time_ms)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (version) DO UPDATE SET
            name = EXCLUDED.name,
            description = EXCLUDED.description,
            checksum = EXCLUDED.checksum,
            execution_time_ms = EXCLUDED.execution_time_ms,
            applied_at = NOW()
    "#)
    .bind(version)
    .bind("initial_setup")
    .bind("Initial database setup with all tables and indexes")
    .bind(checksum)
    .bind(elapsed_ms as i64)
    .execute(pg)
    .await?;

    Ok(())
}

// ── SQLite 迁移逻辑 ───────────────────────────────────────────────

async fn run_sqlite_migrations(pool: &DbPool) {
    let last_version = match pool {
        DbPool::Sqlite(sq) => {
            let result: Result<i64, _> = sqlx::query_scalar(
                "SELECT COALESCE(MAX(version), 0) FROM __migrations"
            )
            .fetch_one(sq)
            .await;

            match result {
                Ok(v) => v,
                Err(_) => 0,
            }
        }
        DbPool::Postgres(_) => 0,
    };

    if last_version >= SQLITE_CURRENT_VERSION {
        info!("[MIGRATION] SQLite up to date. Current version: {}", last_version);
        return;
    }

    info!("[MIGRATION] Running SQLite migration v{} -> v{}", last_version, SQLITE_CURRENT_VERSION);

    let sql_files = collect_sql_files(SQLITE_MIGRATIONS_DIR);
    if sql_files.is_empty() {
        warn!("[MIGRATION] No SQL files found in {}", SQLITE_MIGRATIONS_DIR);
        return;
    }

    for (i, file) in sql_files.iter().enumerate() {
        let relative = file.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join(SQLITE_MIGRATIONS_DIR))
            .unwrap_or(file);
        info!("  {}. {}", i + 1, relative.to_string_lossy());
    }

    let combined_sql = combine_sql_files(&sql_files);
    let start = Instant::now();

    // Enable WAL mode for better concurrency
    if let DbPool::Sqlite(sq) = pool {
        let _ = sqlx::raw_sql("PRAGMA journal_mode=WAL").execute(sq).await;
    }

    if let DbPool::Sqlite(sq) = pool {
        match execute_sqlite_migration(sq, &combined_sql).await {
            Ok(_) => {
                let elapsed = start.elapsed().as_millis();
                let _ = record_sqlite_migration(sq, SQLITE_CURRENT_VERSION).await;
                info!("[MIGRATION] SQLite migration completed in {}ms", elapsed);
            }
            Err(e) => {
                error!("[MIGRATION] SQLite migration failed: {}", e);
            }
        }
    }
}

async fn execute_sqlite_migration(sq: &sqlx::SqlitePool, sql: &str) -> Result<(), sqlx::Error> {
    for stmt in split_sql_statements(sql) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        sqlx::raw_sql(trimmed).execute(sq).await?;
    }
    Ok(())
}

async fn record_sqlite_migration(sq: &sqlx::SqlitePool, version: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR REPLACE INTO __migrations (version, name, description, applied_at) VALUES ($1, $2, $3, datetime('now'))"
    )
    .bind(version)
    .bind("initial_setup")
    .bind("Initial database setup with all tables and indexes")
    .execute(sq)
    .await?;
    Ok(())
}

// ── 共享工具函数 ──────────────────────────────────────────────────

fn collect_sql_files(dir: &str) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);

    if !base.exists() {
        warn!("[MIGRATION] Directory not found: {:?}", base);
        return files;
    }

    collect_recursive(&base, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_recursive(&path, files);
            } else if path.extension().map_or(false, |e| e == "sql") {
                files.push(path);
            }
        }
    }
}

fn combine_sql_files(files: &[std::path::PathBuf]) -> String {
    let mut combined = String::new();
    for file in files {
        if let Ok(content) = std::fs::read_to_string(file) {
            combined.push_str(&content);
            combined.push('\n');
        }
    }
    combined
}

fn calculate_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_sq = false;
    let mut in_dq = false;
    let mut in_comment = false;

    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if !in_sq && !in_dq {
            if c == '-' && i + 1 < chars.len() && chars[i + 1] == '-' {
                in_comment = true;
                i += 2;
                continue;
            }
            if in_comment && c == '\n' {
                in_comment = false;
                i += 1;
                continue;
            }
            if in_comment {
                i += 1;
                continue;
            }
        }

        if c == '\'' && !in_dq {
            in_sq = !in_sq;
            current.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !in_sq {
            in_dq = !in_dq;
            current.push(c);
            i += 1;
            continue;
        }

        if c == ';' && !in_sq && !in_dq {
            statements.push(current.trim().to_string());
            current.clear();
            i += 1;
            continue;
        }

        current.push(c);
        i += 1;
    }

    let last = current.trim();
    if !last.is_empty() {
        statements.push(last.to_string());
    }

    statements
}
