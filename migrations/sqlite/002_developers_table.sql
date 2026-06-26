-- =====================================================================
-- 开发者表 (SQLite)
-- =====================================================================
CREATE TABLE IF NOT EXISTS developers (
    developer_uuid TEXT PRIMARY KEY,
    developer_name VARCHAR(255) NOT NULL UNIQUE,
    successful_auths INTEGER NOT NULL DEFAULT 0,
    risky_marks_available INTEGER NOT NULL DEFAULT 0,
    total_risky_marks_earned INTEGER NOT NULL DEFAULT 0,
    total_risky_marks_used INTEGER NOT NULL DEFAULT 0,
    last_auth_time TEXT,
    auths_needed_for_next_mark INTEGER NOT NULL DEFAULT 0,
    create_time TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    rate_limit_per_second INTEGER NOT NULL DEFAULT 100,
    deduction_available INTEGER NOT NULL DEFAULT 0,
    deduction_limit INTEGER NOT NULL DEFAULT 1000,
    recovery_amount INTEGER NOT NULL DEFAULT 10,
    recovery_interval_secs INTEGER NOT NULL DEFAULT 60,
    last_recovery_time TEXT
);

CREATE INDEX IF NOT EXISTS idx_developers_name ON developers(developer_name);
CREATE INDEX IF NOT EXISTS idx_developers_created ON developers(create_time);
CREATE INDEX IF NOT EXISTS idx_developers_updated ON developers(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_developers_deduction_limit ON developers(deduction_limit);
CREATE INDEX IF NOT EXISTS idx_developers_deduction_available ON developers(deduction_available);
