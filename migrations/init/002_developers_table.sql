-- =====================================================================
-- 开发者表
-- =====================================================================
CREATE TABLE IF NOT EXISTS sl_uuid.developers (
    developer_uuid UUID PRIMARY KEY,
    developer_name VARCHAR(255) NOT NULL,
    successful_auths BIGINT NOT NULL DEFAULT 0,
    risky_marks_available INT NOT NULL DEFAULT 0,
    total_risky_marks_earned INT NOT NULL DEFAULT 0,
    total_risky_marks_used INT NOT NULL DEFAULT 0,
    last_auth_time TIMESTAMP WITH TIME ZONE,
    auths_needed_for_next_mark INT NOT NULL DEFAULT 0,
    create_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    rate_limit_per_second INT NOT NULL DEFAULT 100,
    deduction_available INT NOT NULL DEFAULT 0,
    deduction_limit INT NOT NULL DEFAULT 1000,
    recovery_amount INT NOT NULL DEFAULT 10,
    recovery_interval_secs INT NOT NULL DEFAULT 60,
    last_recovery_time TIMESTAMP WITH TIME ZONE,
    UNIQUE (developer_name)
);

CREATE INDEX IF NOT EXISTS idx_developers_name ON sl_uuid.developers(developer_name);
CREATE INDEX IF NOT EXISTS idx_developers_created ON sl_uuid.developers(create_time);
CREATE INDEX IF NOT EXISTS idx_developers_updated ON sl_uuid.developers(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_developers_deduction_limit ON sl_uuid.developers(deduction_limit);
CREATE INDEX IF NOT EXISTS idx_developers_deduction_available ON sl_uuid.developers(deduction_available);
