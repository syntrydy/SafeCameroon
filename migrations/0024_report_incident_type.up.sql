-- The reporter's own guess at the incident type, if the intake UI asked.
-- Nullable and never authoritative: a reviewer still explicitly chooses the
-- IncidentType when creating a case from this report.
ALTER TABLE reports ADD COLUMN reported_incident_type incident_type;
