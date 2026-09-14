-- Exponential backoff before a RETRYING delivery may be claimed again
-- (crates/domain/src/delivery.rs's RetryPolicy::backoff_after_attempt).
-- NULL means "eligible immediately" (every QUEUED delivery, and every
-- existing row before this migration); PostgresDeliveryRepository::advance_delivery
-- sets it to `now() + <backoff>` whenever a transition leaves a delivery
-- Retrying, computed entirely in SQL so no date/time crate is needed on the
-- Rust side (mirrors ReviewerSessionTokenIssuer/PostgresRateLimiter's own
-- epoch-second arithmetic instead of a wall-clock Rust type).

ALTER TABLE deliveries ADD COLUMN next_attempt_at TIMESTAMPTZ;
