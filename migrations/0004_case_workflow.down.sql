DROP TRIGGER IF EXISTS case_events_immutable ON case_events;
DROP FUNCTION IF EXISTS reject_case_event_mutation();

DROP TABLE IF EXISTS case_events;
DROP TABLE IF EXISTS case_reports;
DROP TABLE IF EXISTS cases;

DROP TYPE IF EXISTS case_event_type;
DROP TYPE IF EXISTS case_status;
DROP TYPE IF EXISTS incident_type;
