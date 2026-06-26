-- =====================================================================
-- 扣款交易表 (SQLite)
-- =====================================================================
CREATE TABLE IF NOT EXISTS deduction_transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    developer_uuid TEXT NOT NULL REFERENCES developers(developer_uuid) ON DELETE CASCADE,
    transaction_token TEXT NOT NULL DEFAULT (lower(hex(randomblob(16)))),
    amount INTEGER NOT NULL CHECK (amount > 0),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'committed', 'cancelled', 'expired')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    confirmed_at TEXT,
    commit_token TEXT,
    UNIQUE (transaction_token)
);

CREATE INDEX IF NOT EXISTS idx_deduction_dev_uuid ON deduction_transactions(developer_uuid);
CREATE INDEX IF NOT EXISTS idx_deduction_token ON deduction_transactions(transaction_token);
CREATE INDEX IF NOT EXISTS idx_deduction_status ON deduction_transactions(status);
CREATE INDEX IF NOT EXISTS idx_deduction_created ON deduction_transactions(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_deduction_dev_status ON deduction_transactions(developer_uuid, status);
CREATE INDEX IF NOT EXISTS idx_deduction_status_created ON deduction_transactions(status, created_at DESC);
