# Prompt 02 - Database

Read `docs/DATABASE.md` and `docs/DOMAIN_MODEL.md`.

Implement SQLx migrations for the first schema.

Include:
- users
- organizations
- organization_members
- reporters
- contact_endpoints
- consumers
- reports
- report_locations
- report_analyses
- attachments
- cases
- case_reports
- case_events
- alerts
- alert_versions
- subscriptions
- subscription_rules
- subscription_channels
- channel_providers
- deliveries
- delivery_attempts
- outbox_events
- audit_events

Add appropriate foreign keys, indexes, unique constraints, and check constraints.

Keep schema PostgreSQL-portable and document any PostGIS dependency.

Add integration tests that create/migrate the DB and exercise representative relationships.
