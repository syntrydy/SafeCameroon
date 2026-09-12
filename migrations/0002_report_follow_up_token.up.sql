-- Existing reports predate anonymous follow-up. They remain without a token;
-- all reports created after this migration receive a SHA-256 token digest.
ALTER TABLE reports ADD COLUMN follow_up_token_hash BYTEA;

CREATE UNIQUE INDEX reports_follow_up_token_hash_idx
    ON reports (follow_up_token_hash)
    WHERE follow_up_token_hash IS NOT NULL;
