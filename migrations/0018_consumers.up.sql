-- A consumer is a receiver identity (docs/DOMAIN_MODEL.md section 9): the
-- organization or citizen a subscription and its deliveries belong to.
-- ConsumerId (crates/domain/src/ids.rs) already exists as an opaque newtype
-- accepted by every subscription/delivery-preference endpoint, but nothing
-- has ever registered or validated one -- this is the first table backing
-- it (docs/API.md section 5: POST /v1/consumers, GET /v1/consumers/{id}).

CREATE TYPE consumer_type AS ENUM ('ORGANIZATION', 'CITIZEN');

CREATE TABLE consumers (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    consumer_type consumer_type NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT consumers_name_is_not_blank CHECK (length(btrim(name)) > 0)
);
