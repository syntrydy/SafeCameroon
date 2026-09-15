-- Mirrors reports_idempotency_key_hash_idx (migrations/0003): a retried
-- POST /v1/cases/{id}/alerts request must produce one alert, not two.
-- DeliveryIdempotencyKey (crates/domain/src/delivery.rs) is keyed on
-- alert_id, so it cannot catch a duplicate *alert* -- two distinct alerts
-- from a retried request independently trigger subscription matching and
-- produce duplicate, non-deduplicated deliveries to every matched consumer.

ALTER TABLE alerts ADD COLUMN idempotency_key_hash BYTEA;

CREATE UNIQUE INDEX alerts_idempotency_key_hash_idx
    ON alerts (idempotency_key_hash)
    WHERE idempotency_key_hash IS NOT NULL;
