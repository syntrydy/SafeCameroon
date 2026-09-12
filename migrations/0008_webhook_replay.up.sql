-- Backs PostgresWebhookReplayGuard: a provider webhook callback is
-- deduplicated by (channel, provider_event_id) so an at-least-once callback
-- delivery never applies the same status transition twice (docs/EVENTS.md
-- section 7). Scoped by channel so two different providers can never
-- collide on the same event id string.

CREATE TABLE webhook_replay_events (
    channel channel_type NOT NULL,
    provider_event_id TEXT NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel, provider_event_id),
    CONSTRAINT webhook_replay_events_provider_event_id_is_not_blank CHECK (
        length(btrim(provider_event_id)) > 0
    )
);

-- PostgresDeliveryRepository::find_by_provider_message_id looks up the
-- delivery a webhook callback's provider_message_id refers to.
CREATE INDEX delivery_attempts_provider_message_id_idx
    ON delivery_attempts (provider_message_id)
    WHERE provider_message_id IS NOT NULL;
