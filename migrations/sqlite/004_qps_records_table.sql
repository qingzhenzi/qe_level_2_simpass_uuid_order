-- =====================================================================
-- QPS 记录表 (SQLite)
-- =====================================================================
CREATE TABLE IF NOT EXISTS qps_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
    total_qps INTEGER NOT NULL DEFAULT 0 CHECK (total_qps >= 0),
    api_path TEXT NOT NULL DEFAULT '',
    developer_uuid TEXT
);

CREATE INDEX IF NOT EXISTS idx_qps_recorded_at ON qps_records(recorded_at);
CREATE INDEX IF NOT EXISTS idx_qps_recorded_at_desc ON qps_records(recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_qps_api_path ON qps_records(api_path);
CREATE INDEX IF NOT EXISTS idx_qps_path_recorded ON qps_records(api_path, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_qps_dev_uuid ON qps_records(developer_uuid);
