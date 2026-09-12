-- Severity is assigned by the reviewer at alert-creation time (prompt 06);
-- the case workflow has no severity-assignment step of its own yet, so this
-- column lives on alerts, not cases. `trigger` records which case event
-- produced this specific alert, reusing the existing case_event_type enum
-- so the subscription engine's EventType rule can match against it directly.

CREATE TYPE severity_level AS ENUM ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL');

ALTER TABLE alerts ADD COLUMN severity severity_level NOT NULL DEFAULT 'MEDIUM';
ALTER TABLE alerts ALTER COLUMN severity DROP DEFAULT;

ALTER TABLE alerts ADD COLUMN trigger case_event_type NOT NULL DEFAULT 'CASE_VERIFIED';
ALTER TABLE alerts ALTER COLUMN trigger DROP DEFAULT;

CREATE INDEX alerts_severity_idx ON alerts (severity);
