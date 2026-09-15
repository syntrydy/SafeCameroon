DROP INDEX alerts_idempotency_key_hash_idx;
ALTER TABLE alerts DROP COLUMN idempotency_key_hash;
