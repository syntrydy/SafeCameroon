-- Links a reviewer Organization (crates/domain/src/organization.rs) to the
-- Consumer alerts are actually delivered through (crates/domain/src/consumer.rs)
-- so an org admin can manage their own organization's alert subscription and
-- delivery preference (WhatsApp/email endpoints) through the same tested
-- subscription/delivery pipeline every other consumer already uses, rather
-- than a second parallel notification mechanism living on Organization
-- itself. Nullable: only organizations created from here on get one
-- auto-linked at creation time (apps/api/src/organizations.rs); organizations
-- that predate this migration are unaffected until backfilled.

-- UNIQUE: a consumer is owned by at most one organization, so authorization
-- can look up "which organization owns this consumer" as a single row
-- (apps/api/src/subscriptions.rs's org-scoped consumer management check).
ALTER TABLE organizations ADD COLUMN consumer_id UUID UNIQUE REFERENCES consumers (id);
