-- Reviewer accounts (prompt 09, docs/SECURITY_PRIVACY.md section 1:
-- "authenticated platform access" as a distinct identity boundary from
-- anonymous reporting). This is deliberately the first user table
-- (migrations/README.md: "the first migration deliberately does not create
-- organization or user tables. Those will arrive with the authorization
-- foundation") — there is still no organization/role table, matching the
-- flat `Capability` model in crates/application/src/authorization.rs.
--
-- password_hash stores a self-describing Argon2id PHC string (algorithm,
-- salt, and parameters travel with the hash); nothing here ever sees or
-- stores a plaintext password.

CREATE TABLE reviewers (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT reviewers_email_is_not_blank CHECK (length(btrim(email)) > 0),
    CONSTRAINT reviewers_password_hash_is_not_blank CHECK (length(btrim(password_hash)) > 0)
);

CREATE UNIQUE INDEX reviewers_email_unique_idx ON reviewers (lower(email));
