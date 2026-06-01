use sqlx::{PgPool, Row};
use log::{info, warn, error};
use sha2::{Sha256, Digest};
use std::time::Instant;
use std::path::Path;

const CURRENT_VERSION: i64 = 1;
const MIGRATION_NAME: &str = "initial_setup";
const MIGRATION_DESCRIPTION: &str = "Initial database setup with all tables and indexes";

const MIGRATIONS_DIR: &str = "../../migrations";

/// 运行数据库迁移
pub async fn run_migrations(pool: &PgPool) {
    ensure_migration_table_exists(pool).await;

    let last_version = get_last_migration_version(pool).await;
    
    if last_version >= CURRENT_VERSION {
        info!("[MIGRATION] No new migrations to apply. Current version: {}", last_version);
        validate_directory_checksum(pool, CURRENT_VERSION).await;
        return;
    }

    info!("[MIGRATION] Starting database migration from version {} to {}", last_version, CURRENT_VERSION);
    
    let sql_files = collect_sql_files();
    
    if sql_files.is_empty() {
        info!("[MIGRATION] No SQL files found in {}", MIGRATIONS_DIR);
        return;
    }
    
    info!("[MIGRATION] Found {} SQL files to execute:", sql_files.len());
    for (i, file) in sql_files.iter().enumerate() {
        let relative = file.strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join(MIGRATIONS_DIR))
            .unwrap_or(file);
        info!("  {}. {}", i + 1, relative.to_string_lossy());
    }

    let combined_sql = combine_sql_files(&sql_files);
    let checksum = calculate_checksum(&combined_sql);
    
    let start_time = Instant::now();
    match execute_migration(pool, &combined_sql).await {
        Ok(_) => {
            let execution_time = start_time.elapsed().as_millis();
            
            if let Err(e) = record_migration(
                pool, 
                CURRENT_VERSION, 
                MIGRATION_NAME, 
                MIGRATION_DESCRIPTION, 
                &checksum,
                execution_time
            ).await {
                warn!("[MIGRATION] Migration executed but failed to record: {}", e);
            }
            
            info!("[MIGRATION] Migration '{}' completed successfully in {}ms ({} files)", 
                  MIGRATION_NAME, execution_time, sql_files.len());
        }
        Err(e) => {
            error!("[MIGRATION] Migration '{}' failed: {}", MIGRATION_NAME, e);
        }
    }
}

/// 递归收集 migrations 目录下所有 .sql 文件并按路径排序
fn collect_sql_files() -> Vec<std::path::PathBuf> {
    let mut sql_files = Vec::new();
    
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(MIGRATIONS_DIR);
    
    if !base_path.exists() {
        warn!("[MIGRATION] Migration directory not found: {:?}", base_path);
        return sql_files;
    }
    
    collect_sql_files_recursive(&base_path, &base_path, &mut sql_files);
    
    sql_files.sort_by(|a, b| {
        a.to_string_lossy().cmp(&b.to_string_lossy())
    });
    
    sql_files
}

/// 递归扫描目录
fn collect_sql_files_recursive(base: &Path, current: &Path, files: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_sql_files_recursive(base, &path, files);
            } else if path.extension().map_or(false, |ext| ext == "sql") {
                files.push(path);
            }
        }
    }
}

/// 合并多个 SQL 文件内容
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

/// 确保迁移跟踪表存在并具有正确的结构
async fn ensure_migration_table_exists(pool: &PgPool) {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'sl_uuid' AND table_name = '__migrations')"
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    
    if exists {
        let has_version: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM information_schema.columns WHERE table_schema = 'sl_uuid' AND table_name = '__migrations' AND column_name = 'version')"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(false);
        
        if !has_version {
            info!("[MIGRATION] Updating existing migration tracking table structure...");
            let alter_result = sqlx::raw_sql(r#"
                ALTER TABLE sl_uuid.__migrations 
                ADD COLUMN IF NOT EXISTS version BIGINT,
                ADD COLUMN IF NOT EXISTS description TEXT,
                ADD COLUMN IF NOT EXISTS checksum VARCHAR(64),
                ADD COLUMN IF NOT EXISTS applied_by VARCHAR(255),
                ADD COLUMN IF NOT EXISTS execution_time_ms BIGINT;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_migrations_version ON sl_uuid.__migrations(version);
                CREATE INDEX IF NOT EXISTS idx_migrations_applied_at ON sl_uuid.__migrations(applied_at DESC);
            "#).execute(pool).await;
            
            match alter_result {
                Ok(_) => info!("[MIGRATION] Migration tracking table updated successfully"),
                Err(e) => warn!("[MIGRATION] Failed to update migration tracking table: {}", e),
            }
            
            let update_result = sqlx::query("UPDATE sl_uuid.__migrations SET version = 0 WHERE version IS NULL").execute(pool).await;
            match update_result {
                Ok(_) => info!("[MIGRATION] Set default version for existing migrations"),
                Err(e) => warn!("[MIGRATION] Failed to set default version: {}", e),
            }
        } else {
            info!("[MIGRATION] Migration tracking table already has correct structure");
        }
        return;
    }
    
    let create_table_sql = r#"
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
    
    match sqlx::raw_sql(create_table_sql).execute(pool).await {
        Ok(_) => info!("[MIGRATION] Migration tracking table created successfully"),
        Err(e) => error!("[MIGRATION] Failed to create migration tracking table: {}", e),
    }
}

/// 获取最后一个迁移版本
async fn get_last_migration_version(pool: &PgPool) -> i64 {
    let result = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(version), 0) FROM sl_uuid.__migrations"
    )
    .fetch_one(pool)
    .await;
    
    match result {
        Ok(Some(version)) => version,
        Ok(None) => 0,
        Err(_) => 0,
    }
}

/// 计算 SQL 内容的 SHA256 哈希
fn calculate_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)
}

/// 验证迁移 checksum 是否匹配
async fn validate_directory_checksum(pool: &PgPool, version: i64) {
    let stored_checksum: Option<String> = sqlx::query_scalar(
        "SELECT checksum FROM sl_uuid.__migrations WHERE version = $1"
    )
    .bind(version)
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    
    if let Some(stored) = stored_checksum {
        let sql_files = collect_sql_files();
        let current_sql = combine_sql_files(&sql_files);
        let current_checksum = calculate_checksum(&current_sql);
        
        if stored != current_checksum {
            warn!("[MIGRATION] Checksum mismatch for migration version {}! Stored: {}, Current: {}", 
                  version, &stored[..8], &current_checksum[..8]);
        } else {
            info!("[MIGRATION] Checksum validation passed for migration version {}", version);
        }
    }
}

/// 执行迁移 SQL
async fn execute_migration(pool: &PgPool, sql: &str) -> Result<(), sqlx::Error> {
    let statements = split_sql_statements(sql);
    let mut executed = 0;
    
    for stmt in statements.iter() {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        
        executed += 1;
        info!("[MIGRATION] Executing statement {}...", executed);
        sqlx::raw_sql(trimmed).execute(pool).await?;
    }
    
    info!("[MIGRATION] Executed {} statements", executed);
    Ok(())
}

/// 智能分割 SQL 语句（处理字符串和注释中的分号）
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut in_comment = false;
    
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let c = chars[i];
        
        if !in_single_quote && !in_double_quote {
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
        
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(c);
            i += 1;
            continue;
        }
        
        if c == ';' && !in_single_quote && !in_double_quote {
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

/// 记录迁移到跟踪表
async fn record_migration(
    pool: &PgPool,
    version: i64,
    name: &str,
    description: &str,
    checksum: &str,
    execution_time_ms: u128
) -> Result<(), sqlx::Error> {
    let _ = sqlx::raw_sql(
        "ALTER TABLE sl_uuid.__migrations DROP CONSTRAINT IF EXISTS __migrations_name_key"
    ).execute(pool).await;
    
    sqlx::query(
        r#"
        INSERT INTO sl_uuid.__migrations 
        (version, name, description, checksum, execution_time_ms)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (version) DO UPDATE SET 
            name = EXCLUDED.name,
            description = EXCLUDED.description,
            checksum = EXCLUDED.checksum,
            execution_time_ms = EXCLUDED.execution_time_ms,
            applied_at = NOW()
        "#
    )
    .bind(version)
    .bind(name)
    .bind(description)
    .bind(checksum)
    .bind(execution_time_ms as i64)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// 获取迁移历史记录
pub async fn get_migration_history(pool: &PgPool) -> Result<Vec<MigrationRecord>, sqlx::Error> {
    let mut records = Vec::new();
    
    let rows = sqlx::raw_sql(
        "SELECT id, version, name, description, checksum, applied_at, applied_by, execution_time_ms FROM sl_uuid.__migrations ORDER BY version DESC"
    )
    .fetch_all(pool)
    .await?;
    
    for row in rows {
        records.push(MigrationRecord {
            id: row.try_get("id")?,
            version: row.try_get("version")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            checksum: row.try_get("checksum")?,
            applied_at: row.try_get("applied_at")?,
            applied_by: row.try_get("applied_by")?,
            execution_time_ms: row.try_get("execution_time_ms")?,
        });
    }
    
    Ok(records)
}

/// 迁移记录结构体
#[derive(Debug, Clone)]
pub struct MigrationRecord {
    pub id: i64,
    pub version: i64,
    pub name: String,
    pub description: Option<String>,
    pub checksum: Option<String>,
    pub applied_at: chrono::DateTime<chrono::Utc>,
    pub applied_by: Option<String>,
    pub execution_time_ms: Option<i64>,
}
