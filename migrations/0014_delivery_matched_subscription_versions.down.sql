ALTER TABLE deliveries DROP CONSTRAINT deliveries_matching_subscriptions_is_an_array;
ALTER TABLE deliveries DROP COLUMN matching_subscriptions;
ALTER TABLE deliveries ADD COLUMN matching_subscription_ids UUID[] NOT NULL DEFAULT '{}';
