# SafeCameroon

SafeCameroon is a privacy-first civic-protection platform for anonymous incident intake,
case coordination, controlled alerts, and multi-channel delivery. The first operational
use case is missing children.

## Repository layout

- `crates/domain`: pure domain types and state-transition rules.
- `crates/application`: use cases; infrastructure ports belong here as implementation begins.
- `apps/api`: Axum HTTP API.
- `apps/worker`: asynchronous worker that polls and dispatches deliveries.
- `apps/console`: future TypeScript/React organization console scaffold.

## Local checks

Copy `.env.example` to `.env` and fill in real values, or prefix each
command with the vars inline as shown below — both binaries load `.env` at
startup via `dotenvy::dotenv()` if one is present, and otherwise read
straight from the process environment (a real deployment sets vars
directly and has no `.env` at all; a missing file is not an error).

```bash
cargo test --workspace
DATABASE_URL=postgres://... WEBHOOK_SHARED_SECRET=... ATTACHMENT_STORAGE_SECRET=... REVIEWER_SESSION_SECRET=... GOOGLE_OAUTH_CLIENT_ID=... cargo run -p safe-cameroon-api
curl http://localhost:3000/health
DATABASE_URL=postgres://... cargo run -p safe-cameroon-worker
```

The worker polls for `QUEUED`/`RETRYING` deliveries and dispatches them
through mock/sandbox `WhatsAppChannel`/`SmsChannel`/`EmailChannel` adapters
(prompt 08; stdout only, no vendor SDK) until real provider credentials
exist. The API exposes `POST /v1/webhooks/{channel}/{provider}`
(docs/API.md section 7) for provider delivery-status callbacks; today the
only registered provider is `sandbox`, verified with an HMAC-SHA256
signature over `WEBHOOK_SHARED_SECRET` (matches every mock channel adapter's
payload shape).

The worker's other poll loop, subscription matching, claims unpublished
`ALERT_CREATED` outbox events (`outbox_events.claimed_at`) and only marks an
event `published_at` once its own matching against every subscription has
actually succeeded — a failure partway through a batch leaves later events
still claimed rather than silently, permanently marked done, mirroring how
`PostgresDeliveryRepository::claim_next` moves a delivery into the
intermediate `SENDING` status rather than a terminal one before it dispatches.

Attachments (`POST /v1/reports/{report_id}/attachments`,
`GET /v1/attachments/{id}/download-url`) use real Cloudflare R2 when
`R2_ACCOUNT_ID`/`R2_BUCKET_NAME`/`R2_ACCESS_KEY_ID`/`R2_SECRET_ACCESS_KEY`
are all configured — `R2AttachmentStorage` presigns PUT/GET URLs against
R2's S3-compatible endpoint using `aws-sigv4` (the same signing crate the
AWS SDKs use internally), since R2 is API-compatible with S3's SigV4
scheme. Without those vars (local dev, the test suite), it falls back to
`HmacSignedAttachmentStorage`, which issues short-lived, HMAC-SHA256-signed
mock URLs over `ATTACHMENT_STORAGE_SECRET` instead
(`ATTACHMENT_STORAGE_BASE_URL` optionally overrides the placeholder base
URL). Uploading requires no actor (part of anonymous report submission);
requesting a download URL requires an identified reviewer and is audited —
both true regardless of which storage backend is active.

`GET /v1/reports/{report_id}/attachments` lists a report's attachments —
metadata only (id, object key, content type, size, checksum), never the
file bytes or a live download URL; a reviewer still calls the existing
`GET /v1/attachments/{id}/download-url` per attachment for that. Gated by
`Capability::ViewCase`, same as the download-url endpoint; shares the
`/v1/reports/{report_id}/attachments` path with the existing `POST` (upload)
handler.

Reviewer accounts (`POST /v1/auth/register`, `POST /v1/auth/google`) are the
first real authentication in the system: every protected endpoint used to
trust a client-supplied `X-Reviewer-Id` header outright, now it requires an
`Authorization: Bearer <token>` issued by a successful sign-in and verified
with HMAC-SHA256 over `REVIEWER_SESSION_SECRET` (`ReviewerSessionTokenIssuer`,
the same signed-token scheme `HmacSignedAttachmentStorage` uses for
short-lived URLs). Reviewers never have a locally stored password: `POST
/v1/auth/google` takes a Google ID token, verifies it against Google's own
`tokeninfo` endpoint (`GoogleTokenInfoVerifier`,
crates/infrastructure/src/google_identity.rs), and matches the verified
email against the `reviewers` table — an email that verifies but isn't
registered gets `403 REVIEWER_NOT_REGISTERED`. `POST /v1/auth/register` just
allowlists an email; it's open, unauthenticated only to bootstrap a fresh
deployment's very first reviewer, and every registration after that requires
an authenticated reviewer. `POST /v1/auth/logout` revokes every session
currently issued to the calling reviewer ("logout everywhere") — a
compromised or offboarded reviewer no longer has to wait out a token's 12h
expiry.

`POST /v1/reports` and `POST /v1/auth/google` are rate-limited
(docs/SECURITY_PRIVACY.md section 8): a fixed-window counter in Postgres
(`rate_limit_windows`, `PostgresRateLimiter`) refuses a request with
`429 RATE_LIMIT_EXCEEDED` once its source exceeds the scope's limit. Both are
keyed by `X-Forwarded-For` (the real client address once Cloudflare —
docs/DEPLOYMENT.md's edge — is in front; a shared bucket otherwise); sign-in
is keyed by source rather than the (not yet known, until Google verifies the
token) target email, so a burst of garbage tokens is throttled before it can
spend Google's tokeninfo quota. This is
defense-in-depth alongside Cloudflare's own edge rate limiting, not a
replacement for it. `apps/worker` deletes expired `rate_limit_windows` rows
once an hour so the table doesn't grow forever.

`GET /v1/alerts/{id}/deliveries` and `GET /v1/deliveries/{id}` (the latter
including full attempt history — outcome, retryable, failure reason per
attempt) answer docs/OBSERVABILITY.md's "which deliveries were attempted,
and why did one fail?" through the API instead of a direct database query;
gated by `Capability::ViewCase`, the same as attachment download.

`GET /v1/reports` is the only way to read a report's content through the
API — `POST /v1/reports` (anonymous submission) had no matching reader,
so a reviewer had no API path to see what a report says before deciding
whether to open a case. Lists most recently received first
(`reports_status_received_at_idx`), optionally filtered by `status`; same
`limit`/`offset` convention as `GET /v1/audit-events`, gated by
`Capability::ViewCase`; shares the `/v1/reports` path with the existing
`POST` (submission) handler.

`GET /v1/alerts` lists alerts, most recently created first
(`alerts_status_created_at_idx`), optionally filtered by `status` and/or
`visibility` — `alert_fields` can carry non-public detail depending on an
alert's policy/visibility (docs/ALERT_SAFETY.md), so like every other case-
and alert-adjacent read this is gated by `Capability::ViewCase`. Same
`limit`/`offset` convention as `GET /v1/audit-events`. `GET /v1/alerts/{id}`
and `GET /v1/cases/{id}` carry the same gate — they briefly didn't, an
oversight against docs/API.md's "authenticated authorized users only" for
these endpoints, fixed once the listing endpoints above made the gap
visible.

`POST /v1/cases/{id}/alerts` supports the same optional `Idempotency-Key`
header `POST /v1/reports` does (8-256 characters, hashed with SHA-256 before
it reaches persistence, parsed by the shared `idempotency_key_from_headers`
helper both endpoints share): a retried request with the same key returns
`409 IDEMPOTENCY_KEY_REUSED` instead of creating a second alert. This
matters more here than it might look — `DeliveryIdempotencyKey` is keyed on
`alert_id`, so two distinct alerts from one retried request independently
trigger subscription matching and produce two real, non-deduplicated
deliveries to every matched consumer.

`POST /v1/consumers` and `GET /v1/consumers/{id}` register and read back a
consumer — a receiver identity (organization or citizen) subscriptions and
deliveries belong to (docs/DOMAIN_MODEL.md section 9). `ConsumerId` was
previously just an opaque, unvalidated UUID any caller could invent when
hitting the subscription/delivery-preference endpoints; there was no
registration step or lookup at all. Reviewer-managed today (not yet citizen
self-service — matches how `apps/api/src/subscriptions.rs` already treats
subscription management), gated by the same `Capability::ManageSubscriptions`.

`GET /v1/cases` lists cases, most recently updated first
(`cases_status_updated_at_idx`), optionally narrowed by `status` and/or
`incident_type` — the only way to find a case without already knowing its
id. Same `limit`/`offset` convention as `GET /v1/audit-events` (default 50,
capped at 100), gated by `Capability::ViewCase`; shares the `/v1/cases`
path with the existing `POST` (case creation) handler.

`GET /v1/cases/{id}/events` returns a case's full lifecycle history —
every transition recorded in `case_events` (creation, report links, review
steps) in chronological order, not just the current status `GET /v1/cases/{id}`
already returns. Same read-only, `Capability::ViewCase`-gated shape as
delivery visibility; shares the `/v1/cases/{id}/events` path with the
existing `POST` (case-transition) handler.

`GET /v1/audit-events` (docs/SECURITY_PRIVACY.md section 6) reads the
generic `audit_events` table that every other endpoint only ever writes to,
filterable by `resource_type`, `resource_id`, `action`, and `actor_id`, most
recent first, `limit` (default 50, capped at 100 regardless of what is
asked for) and `offset` for paging; gated by `Capability::ViewAudit`, which
existed since prompt 09 but had no reader wired to it until now.

Both binaries log structured, JSON-free `tracing` output (`RUST_LOG`
overrides the `info` default, e.g. `RUST_LOG=debug`). Every API request
carries a `x-request-id` header — the client's own value if it sent one,
otherwise a generated one, always echoed back on the response — and that
same id is the `request_id` recorded on the corresponding audit event,
domain event, and any error response (docs/OBSERVABILITY.md section 2).

`apps/api/src/main.rs`'s `full_missing_child_scenario_from_anonymous_report_to_resolution`
test (docs/TESTING.md section 8, Scenario A) drives the whole missing-child
flow end to end through the real HTTP surface: anonymous report -> case
review -> verified case -> community alert -> subscription match ->
delivery -> the provider's delivered callback -> case resolution.

`docs/` contains local planning material and is intentionally excluded from Git.

Database integration tests are intentionally ignored by default. Run them only
against a dedicated PostgreSQL database whose name contains `test`:

```bash
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test report_submission_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test case_workflow_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test alert_policy_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test delivery_engine_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test webhook_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test attachments_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test audit_events_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test consumers_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test subscription_persistence_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test outbox_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test reviewer_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test rate_limit_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-worker --bins -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-api --bins -- --ignored
```

Run all integration test binaries with `--test-threads=1` if you see spurious
`TRUNCATE` deadlocks; each test truncates the shared schema, so they are not
safe to run concurrently against the same database.
