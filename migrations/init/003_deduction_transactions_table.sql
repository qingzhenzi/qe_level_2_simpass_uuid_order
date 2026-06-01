-- =====================================================================
-- 扣款交易表
-- =====================================================================
CREATE TABLE IF NOT EXISTS sl_uuid.deduction_transactions (
    id BIGSERIAL PRIMARY KEY,
    developer_uuid UUID NOT NULL REFERENCES sl_uuid.developers(developer_uuid) ON DELETE CASCADE,
    transaction_token UUID NOT NULL DEFAULT gen_random_uuid(),
    amount INT NOT NULL CHECK (amount > 0),
    status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'committed', 'cancelled', 'expired')),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    confirmed_at TIMESTAMP WITH TIME ZONE,
    commit_token UUID,
    UNIQUE (transaction_token)
);

CREATE INDEX IF NOT EXISTS idx_deduction_dev_uuid ON sl_uuid.deduction_transactions(developer_uuid);
CREATE INDEX IF NOT EXISTS idx_deduction_token ON sl_uuid.deduction_transactions(transaction_token);
CREATE INDEX IF NOT EXISTS idx_deduction_status ON sl_uuid.deduction_transactions(status);
CREATE INDEX IF NOT EXISTS idx_deduction_expires ON sl_uuid.deduction_transactions(expires_at)
    WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_deduction_created ON sl_uuid.deduction_transactions(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_deduction_dev_status ON sl_uuid.deduction_transactions(developer_uuid, status);
CREATE INDEX IF NOT EXISTS idx_deduction_status_created ON sl_uuid.deduction_transactions(status, created_at DESC);
