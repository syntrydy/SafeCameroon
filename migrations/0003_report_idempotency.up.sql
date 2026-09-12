ALTER TABLE reports ADD COLUMN idempotency_key_hash BYTEA;

CREATE UNIQUE INDEX reports_idempotency_key_hash_idx
    ON reports (idempotency_key_hash)
    WHERE idempotency_key_hash IS NOT NULL;
