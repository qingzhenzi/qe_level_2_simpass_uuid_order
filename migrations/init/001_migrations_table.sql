-- =====================================================================
-- 迁移跟踪表 - 增强版
-- =====================================================================
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
