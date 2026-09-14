-- Subscription and consumer delivery preference persistence
-- (docs/SUBSCRIPTION_ENGINE.md). There is still no persisted Consumer
-- aggregate (docs/OPEN_QUESTIONS.md), so consumer_id carries no foreign key
-- here either, mirroring migration 0007's deliveries table.
--
-- `rules` stores the validated Vec<SubscriptionRule> as a JSON array (one
-- object per rule, tagged by a "rule" discriminator); this keeps the closed
-- Rust rule set (crates/domain/src/subscription.rs) as the single place new
-- rule types are added, without a schema migration per rule type. Candidate
-- filtering beyond a plain consumer_id lookup (section 7: indexed/geographic
-- filtering) is deferred until a query pattern actually needs it.

CREATE TYPE delivery_strategy AS ENUM ('ALL', 'PRIMARY_FALLBACK', 'PRIORITY_LIST');

CREATE TABLE subscriptions (
    id UUID PRIMARY KEY,
    consumer_id UUID NOT NULL,
    version INTEGER NOT NULL,
    rules JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT subscriptions_version_is_positive CHECK (version > 0),
    CONSTRAINT subscriptions_rules_is_a_non_empty_array CHECK (
        jsonb_typeof(rules) = 'array' AND jsonb_array_length(rules) > 0
    )
);

CREATE INDEX subscriptions_consumer_id_idx ON subscriptions (consumer_id);

-- One row per consumer: docs/DOMAIN_MODEL.md/SUBSCRIPTION_ENGINE.md section 9
-- models delivery preference per-consumer, not per-subscription, matching
-- `crates/domain/src/delivery.rs`'s `DeliveryPreference`. `channels` stores
-- the validated Vec<ChannelEndpoint> as a JSON array of
-- {"channel": ..., "address": ...} objects.
CREATE TABLE consumer_delivery_preferences (
    consumer_id UUID PRIMARY KEY,
    strategy delivery_strategy NOT NULL,
    channels JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT consumer_delivery_preferences_channels_is_a_non_empty_array CHECK (
        jsonb_typeof(channels) = 'array' AND jsonb_array_length(channels) > 0
    )
);
