-- Tambah index composite untuk efisiensi cleanup idempotency_keys
-- Digunakan untuk menghapus key yang sudah terlalu lama (TTL)
CREATE INDEX IF NOT EXISTS idx_idempotency_keys_tenant_created_at
    ON idempotency_keys(tenant_id, created_at);

-- Query cleanup (bisa dijalankan via cron/background job):
-- DELETE FROM idempotency_keys
-- WHERE created_at < NOW() - INTERVAL '24 hours'
-- AND tenant_id = current_tenant_id;
