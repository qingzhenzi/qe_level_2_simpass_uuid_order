-- =====================================================================
-- 请求日志表 (SQLite)
-- =====================================================================
CREATE TABLE IF NOT EXISTS request_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    developer_uuid TEXT,
    api_path TEXT NOT NULL,
    method TEXT NOT NULL CHECK (method IN ('GET', 'POST', 'PUT', 'DELETE', 'PATCH')),
    status_code INTEGER NOT NULL CHECK (status_code >= 100 AND status_code < 600),
    processed_at TEXT NOT NULL DEFAULT (datetime('now')),
    latency_ms INTEGER NOT NULL DEFAULT 0 CHECK (latency_ms >= 0),
    client_ip TEXT
);

CREATE INDEX IF NOT EXISTS idx_request_logs_time ON request_logs(processed_at);
CREATE INDEX IF NOT EXISTS idx_request_logs_time_desc ON request_logs(processed_at DESC);
CREATE INDEX IF NOT EXISTS idx_request_logs_dev ON request_logs(developer_uuid);
CREATE INDEX IF NOT EXISTS idx_request_logs_path ON request_logs(api_path);
CREATE INDEX IF NOT EXISTS idx_request_logs_status ON request_logs(status_code);
CREATE INDEX IF NOT EXISTS idx_request_logs_method_path ON request_logs(method, api_path);
CREATE INDEX IF NOT EXISTS idx_request_logs_dev_time ON request_logs(developer_uuid, processed_at DESC);
