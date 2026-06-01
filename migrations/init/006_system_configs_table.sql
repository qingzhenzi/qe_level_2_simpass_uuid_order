-- =====================================================================
-- 系统配置表
-- =====================================================================
CREATE TABLE IF NOT EXISTS sl_uuid.system_configs (
    id BIGSERIAL PRIMARY KEY,
    config_key VARCHAR(255) NOT NULL UNIQUE,
    config_value TEXT NOT NULL,
    config_type VARCHAR(50) NOT NULL DEFAULT 'string'
        CHECK (config_type IN ('string', 'integer', 'boolean', 'json', 'float')),
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE (config_key)
);

CREATE INDEX IF NOT EXISTS idx_system_configs_key ON sl_uuid.system_configs(config_key);
