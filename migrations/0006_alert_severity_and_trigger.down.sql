DROP INDEX IF EXISTS alerts_severity_idx;

ALTER TABLE alerts DROP COLUMN IF EXISTS trigger;
ALTER TABLE alerts DROP COLUMN IF EXISTS severity;

DROP TYPE IF EXISTS severity_level;
