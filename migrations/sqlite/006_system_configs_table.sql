-- =====================================================================
-- 系统配置表 (SQLite)
-- =====================================================================
CREATE TABLE IF NOT EXISTS system_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    config_key TEXT NOT NULL UNIQUE,
    config_value TEXT NOT NULL,
    config_type TEXT NOT NULL DEFAULT 'string'
        CHECK (config_type IN ('string', 'integer', 'boolean', 'json', 'float')),
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (config_key)
);

CREATE INDEX IF NOT EXISTS idx_system_configs_key ON system_configs(config_key);
