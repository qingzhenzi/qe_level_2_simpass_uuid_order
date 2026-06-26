-- =====================================================================
-- SQLite 迁移跟踪表
-- =====================================================================
CREATE TABLE IF NOT EXISTS __migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    checksum VARCHAR(64),
    applied_at TEXT NOT NULL DEFAULT (datetime('now')),
    applied_by VARCHAR(255) DEFAULT 'local',
    execution_time_ms INTEGER
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_migrations_version ON __migrations(version);
CREATE INDEX IF NOT EXISTS idx_migrations_applied_at ON __migrations(applied_at DESC);
