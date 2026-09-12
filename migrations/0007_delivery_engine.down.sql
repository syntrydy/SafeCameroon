DROP TRIGGER IF EXISTS delivery_events_immutable ON delivery_events;
DROP FUNCTION IF EXISTS reject_delivery_event_mutation();

DROP TRIGGER IF EXISTS delivery_attempts_immutable ON delivery_attempts;
DROP FUNCTION IF EXISTS reject_delivery_attempt_mutation();

DROP TABLE IF EXISTS delivery_events;
DROP TABLE IF EXISTS delivery_attempts;
DROP TABLE IF EXISTS deliveries;

DROP TYPE IF EXISTS delivery_attempt_outcome;
DROP TYPE IF EXISTS delivery_event_type;
DROP TYPE IF EXISTS delivery_status;
DROP TYPE IF EXISTS channel_type;
