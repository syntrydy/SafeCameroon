-- The reporting foundation deliberately separates report content from optional
-- reporter contact data. Application code supplies UUIDs; no database extension
-- is required to generate identifiers.

CREATE TYPE reporter_identity_mode AS ENUM ('ANONYMOUS', 'PRIVATE', 'IDENTIFIED');
CREATE TYPE report_source_channel AS ENUM ('WEB', 'SMS', 'WHATSAPP', 'PHONE', 'PARTNER_API');
CREATE TYPE report_status AS ENUM ('RECEIVED', 'UNDER_REVIEW', 'LINKED_TO_CASE', 'CLOSED');

CREATE TABLE reporters (
    id UUID PRIMARY KEY,
    identity_mode reporter_identity_mode NOT NULL,
    -- Contact information must be encrypted before it reaches this column.
    contact_ciphertext BYTEA,
    contact_key_version SMALLINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT reporter_contact_encryption_metadata_is_consistent CHECK (
        (contact_ciphertext IS NULL AND contact_key_version IS NULL)
        OR (contact_ciphertext IS NOT NULL AND contact_key_version IS NOT NULL)
    ),
    CONSTRAINT anonymous_reporters_have_no_contact CHECK (
        identity_mode <> 'ANONYMOUS' OR contact_ciphertext IS NULL
    )
);

CREATE TABLE reports (
    id UUID PRIMARY KEY,
    reporter_id UUID REFERENCES reporters(id) ON DELETE RESTRICT,
    source_channel report_source_channel NOT NULL,
    raw_content TEXT NOT NULL,
    status report_status NOT NULL DEFAULT 'RECEIVED',
    source_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT report_content_is_not_blank CHECK (length(btrim(raw_content)) > 0),
    CONSTRAINT report_source_metadata_is_an_object CHECK (jsonb_typeof(source_metadata) = 'object')
);

CREATE INDEX reports_received_at_idx ON reports (received_at DESC);
CREATE INDEX reports_status_received_at_idx ON reports (status, received_at DESC);
CREATE INDEX reports_reporter_id_idx ON reports (reporter_id) WHERE reporter_id IS NOT NULL;

CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    actor_type TEXT NOT NULL,
    actor_id UUID,
    organization_id UUID,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID,
    request_id UUID,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT audit_actor_type_is_not_blank CHECK (length(btrim(actor_type)) > 0),
    CONSTRAINT audit_action_is_not_blank CHECK (length(btrim(action)) > 0),
    CONSTRAINT audit_resource_type_is_not_blank CHECK (length(btrim(resource_type)) > 0),
    CONSTRAINT audit_metadata_is_an_object CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX audit_events_resource_idx ON audit_events (resource_type, resource_id, occurred_at DESC);
CREATE INDEX audit_events_request_id_idx ON audit_events (request_id) WHERE request_id IS NOT NULL;
CREATE INDEX audit_events_occurred_at_idx ON audit_events (occurred_at DESC);

CREATE OR REPLACE FUNCTION reject_audit_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'audit events are immutable';
END;
$$;

CREATE TRIGGER audit_events_immutable
BEFORE UPDATE OR DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION reject_audit_event_mutation();

CREATE TABLE outbox_events (
    id UUID PRIMARY KEY,
    aggregate_type TEXT NOT NULL,
    aggregate_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    schema_version SMALLINT NOT NULL DEFAULT 1,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    published_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    CONSTRAINT outbox_aggregate_type_is_not_blank CHECK (length(btrim(aggregate_type)) > 0),
    CONSTRAINT outbox_event_type_is_not_blank CHECK (length(btrim(event_type)) > 0),
    CONSTRAINT outbox_schema_version_is_positive CHECK (schema_version > 0),
    CONSTRAINT outbox_attempt_count_is_not_negative CHECK (attempt_count >= 0),
    CONSTRAINT outbox_payload_is_an_object CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX outbox_events_unpublished_idx
    ON outbox_events (created_at)
    WHERE published_at IS NULL;
CREATE INDEX outbox_events_aggregate_idx
    ON outbox_events (aggregate_type, aggregate_id, created_at);
