-- Replaces deliveries.matching_subscription_ids (UUID[]) with
-- matching_subscriptions (JSONB), storing the rule version each matched
-- subscription was actually evaluated under alongside its id
-- (docs/SUBSCRIPTION_ENGINE.md section 11: "historical matching decisions
-- must remain explainable under the rule version that was active at the
-- time"). A bare UUID[] had no way to record that version, so an edit to a
-- subscription (Subscription::update_rules, migration 0011's audit trail)
-- could silently change what an already-planned delivery's match
-- explanation would say if recomputed from the current row.
--
-- No real deployment exists yet (this is still pre-pilot, per README.md),
-- so this is a plain drop-and-recreate rather than an in-place conversion;
-- every prior migration in this repository has been purely additive, and
-- this is the first to change an existing column's shape.

ALTER TABLE deliveries DROP COLUMN matching_subscription_ids;
ALTER TABLE deliveries ADD COLUMN matching_subscriptions JSONB NOT NULL DEFAULT '[]';
ALTER TABLE deliveries ADD CONSTRAINT deliveries_matching_subscriptions_is_an_array
    CHECK (jsonb_typeof(matching_subscriptions) = 'array');
