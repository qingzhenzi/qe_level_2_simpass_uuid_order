-- =====================================================================
-- QPS 记录表
-- =====================================================================
CREATE TABLE IF NOT EXISTS sl_uuid.qps_records (
    id BIGSERIAL PRIMARY KEY,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    total_qps INT NOT NULL DEFAULT 0 CHECK (total_qps >= 0),
    api_path VARCHAR(255) NOT NULL DEFAULT '',
    developer_uuid UUID
);

CREATE INDEX IF NOT EXISTS idx_qps_recorded_at ON sl_uuid.qps_records(recorded_at);
CREATE INDEX IF NOT EXISTS idx_qps_recorded_at_desc ON sl_uuid.qps_records(recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_qps_api_path ON sl_uuid.qps_records(api_path);
CREATE INDEX IF NOT EXISTS idx_qps_path_recorded ON sl_uuid.qps_records(api_path, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_qps_dev_uuid ON sl_uuid.qps_records(developer_uuid);
