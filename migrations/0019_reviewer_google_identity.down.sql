-- Local development only (migrations/README.md): restores the column shape,
-- not real password data, which no longer exists anywhere by the time this
-- would run. Existing rows get a placeholder that can never verify as a real
-- Argon2 hash, so a rolled-back deployment still requires re-registration in
-- practice.

ALTER TABLE reviewers ADD COLUMN password_hash TEXT NOT NULL DEFAULT 'not-a-real-hash-placeholder';
ALTER TABLE reviewers ALTER COLUMN password_hash DROP DEFAULT;
ALTER TABLE reviewers ADD CONSTRAINT reviewers_password_hash_is_not_blank CHECK (length(btrim(password_hash)) > 0);
