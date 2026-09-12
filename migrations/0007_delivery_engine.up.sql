-- A delivery is one attempt-tracking record for getting an already-matched
-- alert to one consumer over one channel endpoint (docs/DOMAIN_MODEL.md
-- section 12, docs/CHANNELS.md). There is no persisted Consumer or
-- Subscription aggregate yet, so consumer_id carries no foreign key; the
-- deliveries themselves are what this migration adds.
--
-- `endpoint_address` is a consumer-supplied contact address (phone number or
-- email) for receiving alerts they opted into, which is a different privacy
-- posture than reporter identity (AGENTS.md); it is not encrypted at rest
-- here, matching "do not overengineer the first release" until a concrete
-- requirement says otherwise.

CREATE TYPE channel_type AS ENUM ('WHATSAPP', 'SMS', 'EMAIL');
CREATE TYPE delivery_status AS ENUM (
    'QUEUED', 'SENDING', 'SENT', 'DELIVERED', 'RETRYING', 'FAILED_PERMANENTLY'
);
CREATE TYPE delivery_event_type AS ENUM (
    'DELIVERY_REQUESTED', 'DELIVERY_STARTED', 'DELIVERY_SENT',
    'DELIVERY_DELIVERED', 'DELIVERY_FAILED', 'DELIVERY_RETRIED'
);
CREATE TYPE delivery_attempt_outcome AS ENUM ('SENT', 'FAILED');

CREATE TABLE deliveries (
    id UUID PRIMARY KEY,
    alert_id UUID NOT NULL REFERENCES alerts (id) ON DELETE RESTRICT,
    consumer_id UUID NOT NULL,
    channel channel_type NOT NULL,
    endpoint_address TEXT NOT NULL,
    tier SMALLINT NOT NULL,
    -- The docs/SUBSCRIPTION_ENGINE.md section 8 dedup key
    -- (consumer_id + alert_id + channel + endpoint), hashed the same way
    -- report idempotency keys are.
    idempotency_key_hash BYTEA NOT NULL,
    matching_subscription_ids UUID[] NOT NULL DEFAULT '{}',
    status delivery_status NOT NULL DEFAULT 'QUEUED',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL,
    aggregate_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT deliveries_endpoint_address_is_not_blank CHECK (length(btrim(endpoint_address)) > 0),
    CONSTRAINT deliveries_tier_is_not_negative CHECK (tier >= 0),
    CONSTRAINT deliveries_attempt_count_is_not_negative CHECK (attempt_count >= 0),
    CONSTRAINT deliveries_max_attempts_is_positive CHECK (max_attempts > 0),
    CONSTRAINT deliveries_aggregate_version_is_positive CHECK (aggregate_version > 0),
    CONSTRAINT deliveries_idempotency_key_hash_unique UNIQUE (idempotency_key_hash)
);

CREATE INDEX deliveries_alert_id_idx ON deliveries (alert_id);
CREATE INDEX deliveries_consumer_id_idx ON deliveries (consumer_id);
CREATE INDEX deliveries_status_idx ON deliveries (status);

CREATE TABLE delivery_attempts (
    id UUID PRIMARY KEY,
    delivery_id UUID NOT NULL REFERENCES deliveries (id) ON DELETE RESTRICT,
    attempt_number INTEGER NOT NULL,
    outcome delivery_attempt_outcome NOT NULL,
    provider_message_id TEXT,
    retryable BOOLEAN,
    failure_reason TEXT,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT delivery_attempts_attempt_number_is_positive CHECK (attempt_number > 0),
    CONSTRAINT delivery_attempts_delivery_id_attempt_number_unique UNIQUE (delivery_id, attempt_number),
    CONSTRAINT delivery_attempts_failure_fields_match_outcome CHECK (
        (outcome = 'SENT' AND retryable IS NULL AND failure_reason IS NULL)
        OR (outcome = 'FAILED' AND retryable IS NOT NULL AND failure_reason IS NOT NULL)
    )
);

CREATE INDEX delivery_attempts_delivery_id_idx ON delivery_attempts (delivery_id, attempt_number);

CREATE OR REPLACE FUNCTION reject_delivery_attempt_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'delivery attempts are immutable';
END;
$$;

CREATE TRIGGER delivery_attempts_immutable
BEFORE UPDATE OR DELETE ON delivery_attempts
FOR EACH ROW EXECUTE FUNCTION reject_delivery_attempt_mutation();

CREATE TABLE delivery_events (
    id UUID PRIMARY KEY,
    delivery_id UUID NOT NULL REFERENCES deliveries (id) ON DELETE RESTRICT,
    event_type delivery_event_type NOT NULL,
    aggregate_version BIGINT NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id UUID,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT delivery_events_actor_type_is_not_blank CHECK (length(btrim(actor_type)) > 0),
    CONSTRAINT delivery_events_aggregate_version_is_positive CHECK (aggregate_version > 0),
    CONSTRAINT delivery_events_delivery_id_aggregate_version_unique UNIQUE (delivery_id, aggregate_version)
);

CREATE INDEX delivery_events_delivery_id_idx ON delivery_events (delivery_id, aggregate_version);

CREATE OR REPLACE FUNCTION reject_delivery_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'delivery events are immutable';
END;
$$;

CREATE TRIGGER delivery_events_immutable
BEFORE UPDATE OR DELETE ON delivery_events
FOR EACH ROW EXECUTE FUNCTION reject_delivery_event_mutation();
