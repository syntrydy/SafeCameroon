DROP TABLE IF EXISTS alert_description_generations;

-- Not reversible: Postgres has no DROP VALUE for enum types, and the two
-- new alert_field values may already be referenced by alert_fields rows.
-- Local dev recovery: drop and recreate the database from migrations
-- instead of rolling back past this file (migrations/README.md: down
-- migrations are a local-dev/tested-recovery convenience, not a production
-- path).
