-- PostgreSQL cannot drop a single value from an enum type (no `ALTER TYPE
-- ... DROP VALUE`), so the PUSH value added by the up migration is not
-- removed here -- consistent with migrations/README.md: down migrations are
-- a local-development convenience, not a guaranteed exact inverse.

ALTER TABLE consumers DROP COLUMN management_token_hash;
