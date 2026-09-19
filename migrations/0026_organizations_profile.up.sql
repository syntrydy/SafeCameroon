-- Organization profile fields (description, physical location) and a
-- soft-deactivate flag (crates/domain/src/organization.rs). Deactivating an
-- organization never deletes it or its memberships/subscriptions -- it just
-- stops the organization's own trust grants, membership grants, and
-- consumer/delivery management from being usable
-- (crates/application/src/authorization.rs), matching the platform's
-- forward-only migration policy (migrations/README.md) and avoiding an
-- irreversible action for something a platform admin may want to undo.
ALTER TABLE organizations ADD COLUMN description TEXT;
ALTER TABLE organizations ADD COLUMN location TEXT;
ALTER TABLE organizations ADD COLUMN is_active BOOLEAN NOT NULL DEFAULT true;
