-- Session revocation (prompt 09, docs/SECURITY_PRIVACY.md section 10:
-- "incident response" for unauthorized access). sessions_revoked_at is a
-- single "logout everywhere as of this moment" marker per reviewer, not a
-- growing per-token revocation list: any session token whose issued_at
-- predates this timestamp is treated as invalid
-- (safe_cameroon_application::reviewer_auth::SessionRevocationStore); a
-- fresh login issued after this moment is unaffected. NULL means no
-- revocation has ever occurred.

ALTER TABLE reviewers ADD COLUMN sessions_revoked_at TIMESTAMPTZ;
