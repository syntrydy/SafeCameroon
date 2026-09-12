-- Cases are the canonical incident record. A case always starts from at least
-- one report; additional reports may be linked later as corroborating or
-- follow-up information (docs/ALERT_SAFETY.md, section 6).

CREATE TYPE incident_type AS ENUM ('MISSING_CHILD', 'OTHER_PROTECTION_INCIDENT');
CREATE TYPE case_status AS ENUM (
    'REPORTED',
    'UNDER_REVIEW',
    'VERIFIED',
    'ACTIVE',
    'RESOLVED',
    'CANCELLED',
    'REJECTED'
);
CREATE TYPE case_event_type AS ENUM (
    'CASE_CREATED',
    'CASE_REPORT_LINKED',
    'CASE_UNDER_REVIEW',
    'CASE_VERIFIED',
    'CASE_ACTIVATED',
    'CASE_RESOLVED',
    'CASE_CANCELLED',
    'CASE_REJECTED'
);

CREATE TABLE cases (
    id UUID PRIMARY KEY,
    incident_type incident_type NOT NULL,
    status case_status NOT NULL DEFAULT 'REPORTED',
    aggregate_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT cases_aggregate_version_is_positive CHECK (aggregate_version > 0)
);

CREATE INDEX cases_status_updated_at_idx ON cases (status, updated_at DESC);
CREATE INDEX cases_incident_type_idx ON cases (incident_type);

-- Link table between cases and their source reports. A report may only ever
-- belong to one case, matching the current one-case-per-report product rule.
CREATE TABLE case_reports (
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE RESTRICT,
    report_id UUID NOT NULL REFERENCES reports (id) ON DELETE RESTRICT,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (case_id, report_id),
    CONSTRAINT case_reports_report_id_unique UNIQUE (report_id)
);

CREATE TABLE case_events (
    id UUID PRIMARY KEY,
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE RESTRICT,
    event_type case_event_type NOT NULL,
    aggregate_version BIGINT NOT NULL,
    -- REVIEWER actions require actor_id; AUTOMATED actions (AI/system triage)
    -- never verify/resolve a case (see prepare_case_review in the application
    -- crate), so actor_id stays NULL for those rows.
    actor_type TEXT NOT NULL,
    actor_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT case_events_actor_type_is_not_blank CHECK (length(btrim(actor_type)) > 0),
    CONSTRAINT case_events_aggregate_version_is_positive CHECK (aggregate_version > 0),
    CONSTRAINT case_events_case_id_aggregate_version_unique UNIQUE (case_id, aggregate_version)
);

CREATE INDEX case_events_case_id_idx ON case_events (case_id, aggregate_version);

CREATE OR REPLACE FUNCTION reject_case_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'case events are immutable';
END;
$$;

CREATE TRIGGER case_events_immutable
BEFORE UPDATE OR DELETE ON case_events
FOR EACH ROW EXECUTE FUNCTION reject_case_event_mutation();
