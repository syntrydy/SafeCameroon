-- Fixed-window per-source rate limiting (docs/SECURITY_PRIVACY.md section 8,
-- crates/application/src/rate_limit.rs). window_start_epoch_seconds is a
-- plain Unix timestamp rather than TIMESTAMPTZ so the window boundary can be
-- computed with integer arithmetic in application code
-- (`now - (now % window_seconds)`) without a date/time crate dependency.
--
-- Old rows are never deleted by this migration; a scheduled cleanup job is a
-- deliberate follow-up (the table only ever grows by (scope, source_key,
-- window) triples, which is bounded and small for the two scopes that exist
-- today).

CREATE TABLE rate_limit_windows (
    scope TEXT NOT NULL,
    source_key TEXT NOT NULL,
    window_start_epoch_seconds BIGINT NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (scope, source_key, window_start_epoch_seconds),
    CONSTRAINT rate_limit_windows_scope_is_not_blank CHECK (length(btrim(scope)) > 0),
    CONSTRAINT rate_limit_windows_source_key_is_not_blank CHECK (length(btrim(source_key)) > 0),
    CONSTRAINT rate_limit_windows_request_count_is_positive CHECK (request_count > 0)
);
