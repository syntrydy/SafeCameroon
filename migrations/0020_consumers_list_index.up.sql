-- Supports PostgresConsumerRepository::list's "most recently registered
-- first" ordering, mirroring reports_status_received_at_idx /
-- cases_status_updated_at_idx for the same kind of listing endpoint.

CREATE INDEX consumers_created_at_idx ON consumers (created_at DESC);
