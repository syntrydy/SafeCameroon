-- An alert is a controlled, policy-scoped projection of a verified case
-- (docs/ALERT_SAFETY.md). It never stores raw report content or reporter
-- identity; alert_fields only ever holds values for the closed alert_field
-- vocabulary, and REPORTER_IDENTITY is rejected outright at the database
-- layer as well as in the application (defense in depth).

CREATE TYPE alert_visibility AS ENUM ('INTERNAL', 'PARTNER', 'COMMUNITY', 'PUBLIC');
CREATE TYPE alert_status AS ENUM ('ACTIVE', 'CANCELLED');
CREATE TYPE alert_event_type AS ENUM ('ALERT_CREATED', 'ALERT_CANCELLED');
CREATE TYPE alert_field AS ENUM (
    'INCIDENT_CATEGORY',
    'APPROXIMATE_AGE',
    'LAST_SEEN_GENERAL_AREA',
    'TIME_WINDOW',
    'SAFE_DESCRIPTION',
    'OFFICIAL_CONTACT',
    'CASE_REFERENCE',
    'REPORTER_IDENTITY',
    'INTERNAL_NOTES',
    'EXACT_LOCATION',
    'WITNESS_DETAILS'
);

CREATE TABLE alerts (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE RESTRICT,
    policy_id TEXT NOT NULL,
    policy_version SMALLINT NOT NULL,
    incident_type incident_type NOT NULL,
    visibility alert_visibility NOT NULL,
    target_geography TEXT NOT NULL,
    status alert_status NOT NULL DEFAULT 'ACTIVE',
    aggregate_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT alerts_policy_id_is_not_blank CHECK (length(btrim(policy_id)) > 0),
    CONSTRAINT alerts_policy_version_is_positive CHECK (policy_version > 0),
    CONSTRAINT alerts_target_geography_is_not_blank CHECK (length(btrim(target_geography)) > 0),
    CONSTRAINT alerts_aggregate_version_is_positive CHECK (aggregate_version > 0)
);

CREATE INDEX alerts_case_id_idx ON alerts (case_id);
CREATE INDEX alerts_status_created_at_idx ON alerts (status, created_at DESC);
CREATE INDEX alerts_visibility_idx ON alerts (visibility);

CREATE TABLE alert_fields (
    alert_id UUID NOT NULL REFERENCES alerts (id) ON DELETE RESTRICT,
    field alert_field NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (alert_id, field),
    CONSTRAINT alert_fields_value_is_not_blank CHECK (length(btrim(value)) > 0),
    CONSTRAINT alert_fields_never_reporter_identity CHECK (field <> 'REPORTER_IDENTITY')
);

CREATE TABLE alert_events (
    id UUID PRIMARY KEY,
    alert_id UUID NOT NULL REFERENCES alerts (id) ON DELETE RESTRICT,
    event_type alert_event_type NOT NULL,
    aggregate_version BIGINT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT alert_events_actor_type_is_not_blank CHECK (length(btrim(actor_type)) > 0),
    CONSTRAINT alert_events_aggregate_version_is_positive CHECK (aggregate_version > 0),
    CONSTRAINT alert_events_alert_id_aggregate_version_unique UNIQUE (alert_id, aggregate_version)
);

CREATE INDEX alert_events_alert_id_idx ON alert_events (alert_id, aggregate_version);

CREATE OR REPLACE FUNCTION reject_alert_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'alert events are immutable';
END;
$$;

CREATE TRIGGER alert_events_immutable
BEFORE UPDATE OR DELETE ON alert_events
FOR EACH ROW EXECUTE FUNCTION reject_alert_event_mutation();
