-- Organizations and reviewer roles (crates/domain/src/organization.rs;
-- docs/OPEN_QUESTIONS.md governance section). This replaces the flat "any
-- identified reviewer holds every capability" model with real organization
-- membership: PLATFORM_ADMIN is org-independent; ORG_ADMIN and MEMBER each
-- belong to exactly one organization.
--
-- verified_incident_types/verified_alert_visibilities are the explicit
-- trust grants a PLATFORM_ADMIN sets per real organization at onboarding
-- time (which incident types it may verify, which alert visibilities it may
-- issue) -- a deliberate per-deployment configuration point rather than a
-- policy this codebase bakes in, per organization.rs's module doc comment.
-- Plain TEXT[] rather than join tables: each list is small (2 and 4
-- possible values today) and always replaced wholesale, never queried by
-- individual grant.

CREATE TYPE reviewer_role AS ENUM ('PLATFORM_ADMIN', 'ORG_ADMIN', 'MEMBER');

CREATE TABLE organizations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    verified_incident_types TEXT[] NOT NULL DEFAULT '{}',
    verified_alert_visibilities TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT organizations_name_is_not_blank CHECK (length(btrim(name)) > 0)
);

CREATE INDEX organizations_created_at_idx ON organizations (created_at DESC);

-- One row per reviewer (at most one organization membership per reviewer
-- for this first release -- a reviewer who genuinely serves two real
-- organizations is not a case the pilot needs to support yet).
CREATE TABLE reviewer_organization_memberships (
    reviewer_id UUID PRIMARY KEY REFERENCES reviewers (id),
    role reviewer_role NOT NULL,
    organization_id UUID REFERENCES organizations (id),
    CONSTRAINT platform_admin_has_no_organization CHECK (
        (role = 'PLATFORM_ADMIN' AND organization_id IS NULL)
        OR (role != 'PLATFORM_ADMIN' AND organization_id IS NOT NULL)
    )
);

-- Every reviewer who already existed before this migration was, under the
-- old flat model, implicitly trusted with every capability -- grandfather
-- them in as PLATFORM_ADMIN so nobody already provisioned is locked out of
-- registering colleagues, verifying cases, or issuing alerts the moment
-- this migration applies. A no-op on a fresh deployment with no reviewers
-- yet.
INSERT INTO reviewer_organization_memberships (reviewer_id, role, organization_id)
SELECT id, 'PLATFORM_ADMIN', NULL FROM reviewers
ON CONFLICT (reviewer_id) DO NOTHING;
