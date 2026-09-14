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

```bash
cargo test --workspace
DATABASE_URL=postgres://... WEBHOOK_SHARED_SECRET=... ATTACHMENT_STORAGE_SECRET=... REVIEWER_SESSION_SECRET=... cargo run -p safe-cameroon-api
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

Attachments (`POST /v1/reports/{report_id}/attachments`,
`GET /v1/attachments/{id}/download-url`) work the same way: no real R2
credentials exist yet, so `HmacSignedAttachmentStorage` issues short-lived,
HMAC-SHA256-signed mock URLs over `ATTACHMENT_STORAGE_SECRET`
(`ATTACHMENT_STORAGE_BASE_URL` optionally overrides the placeholder base
URL). Uploading requires no actor (part of anonymous report submission);
requesting a download URL requires an identified reviewer and is audited.

Reviewer accounts (`POST /v1/auth/register`, `POST /v1/auth/login`) are the
first real authentication in the system: every protected endpoint used to
trust a client-supplied `X-Reviewer-Id` header outright, now it requires an
`Authorization: Bearer <token>` issued by a successful login and verified
with HMAC-SHA256 over `REVIEWER_SESSION_SECRET` (`ReviewerSessionTokenIssuer`,
the same signed-token scheme `HmacSignedAttachmentStorage` uses for
short-lived URLs). Registration is open, unauthenticated only to bootstrap a
fresh deployment's very first reviewer; every registration after that
requires an authenticated reviewer. `POST /v1/auth/logout` revokes every
session currently issued to the calling reviewer ("logout everywhere") —
a compromised or offboarded reviewer no longer has to wait out a token's
12h expiry.

`POST /v1/reports` and `POST /v1/auth/login` are rate-limited
(docs/SECURITY_PRIVACY.md section 8): a fixed-window counter in Postgres
(`rate_limit_windows`, `PostgresRateLimiter`) refuses a request with
`429 RATE_LIMIT_EXCEEDED` once its source exceeds the scope's limit.
Reporting is keyed by `X-Forwarded-For` (the real client address once
Cloudflare — docs/DEPLOYMENT.md's edge — is in front; a shared bucket
otherwise); login is keyed by the targeted email so credential stuffing
against one account is throttled even across rotating source IPs. This is
defense-in-depth alongside Cloudflare's own edge rate limiting, not a
replacement for it.

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
