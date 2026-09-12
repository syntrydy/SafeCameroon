DROP TRIGGER IF EXISTS alert_events_immutable ON alert_events;
DROP FUNCTION IF EXISTS reject_alert_event_mutation();

DROP TABLE IF EXISTS alert_events;
DROP TABLE IF EXISTS alert_fields;
DROP TABLE IF EXISTS alerts;

DROP TYPE IF EXISTS alert_field;
DROP TYPE IF EXISTS alert_event_type;
DROP TYPE IF EXISTS alert_status;
DROP TYPE IF EXISTS alert_visibility;
