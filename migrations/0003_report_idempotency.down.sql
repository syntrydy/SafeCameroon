DROP INDEX reports_idempotency_key_hash_idx;
ALTER TABLE reports DROP COLUMN idempotency_key_hash;
