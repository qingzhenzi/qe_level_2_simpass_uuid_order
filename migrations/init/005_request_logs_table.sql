-- =====================================================================
-- 请求日志表
-- =====================================================================
CREATE TABLE IF NOT EXISTS sl_uuid.request_logs (
    id BIGSERIAL PRIMARY KEY,
    developer_uuid UUID,
    api_path VARCHAR(255) NOT NULL,
    method VARCHAR(10) NOT NULL CHECK (method IN ('GET', 'POST', 'PUT', 'DELETE', 'PATCH')),
    status_code INT NOT NULL CHECK (status_code >= 100 AND status_code < 600),
    processed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    latency_ms INT NOT NULL DEFAULT 0 CHECK (latency_ms >= 0),
    client_ip INET
);

CREATE INDEX IF NOT EXISTS idx_request_logs_time ON sl_uuid.request_logs(processed_at);
CREATE INDEX IF NOT EXISTS idx_request_logs_time_desc ON sl_uuid.request_logs(processed_at DESC);
CREATE INDEX IF NOT EXISTS idx_request_logs_dev ON sl_uuid.request_logs(developer_uuid);
CREATE INDEX IF NOT EXISTS idx_request_logs_path ON sl_uuid.request_logs(api_path);
CREATE INDEX IF NOT EXISTS idx_request_logs_status ON sl_uuid.request_logs(status_code);
CREATE INDEX IF NOT EXISTS idx_request_logs_method_path ON sl_uuid.request_logs(method, api_path);
CREATE INDEX IF NOT EXISTS idx_request_logs_dev_time ON sl_uuid.request_logs(developer_uuid, processed_at DESC);
