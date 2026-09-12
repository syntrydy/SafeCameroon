DROP TABLE outbox_events;

DROP TRIGGER audit_events_immutable ON audit_events;
DROP FUNCTION reject_audit_event_mutation();
DROP TABLE audit_events;

DROP TABLE reports;
DROP TABLE reporters;

DROP TYPE report_status;
DROP TYPE report_source_channel;
DROP TYPE reporter_identity_mode;
