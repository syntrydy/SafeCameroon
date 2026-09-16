-- Adds the two things a self-service, anonymous citizen subscription needs
-- that a reviewer-managed one never did (apps/api/src/citizen_subscriptions.rs):
--
-- 1. A new PUSH channel (crates/domain/src/delivery.rs) -- Web Push is the
--    only channel a citizen can self-subscribe to today, since ownership is
--    inherent to the browser's own subscription (docs/OPEN_QUESTIONS.md:
--    WhatsApp/SMS provider and verification are still undecided).
-- 2. `management_token_hash` on `consumers`: a self-service consumer has no
--    reviewer account to authenticate later edits/cancellation with, so it
--    is instead given a private, unguessable token at creation (mirrors
--    reports' reference_code -- only the SHA-256 digest is ever stored).
--    NULL for every reviewer-managed consumer, which is every consumer that
--    existed before this migration.

ALTER TYPE channel_type ADD VALUE 'PUSH';

ALTER TABLE consumers ADD COLUMN management_token_hash BYTEA;
