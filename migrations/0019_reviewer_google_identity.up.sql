-- Reviewers now authenticate via Google sign-in (a verified Google ID
-- token) instead of a locally stored credential, so there is nothing left
-- to hash or store here — `email` alone (already the unique identity column)
-- is what a verified Google login is matched against.

ALTER TABLE reviewers DROP CONSTRAINT reviewers_password_hash_is_not_blank;
ALTER TABLE reviewers DROP COLUMN password_hash;
