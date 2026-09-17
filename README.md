# Sentinel

_(Repository name is `SafeCameroon` for historical reasons — Cameroon was the pilot country;
the product itself is named Sentinel and is built to run in any country. See
["Multi-country deployment model"](#multi-country-deployment-model) below.)_

Sentinel is a privacy-first civic-protection platform for anonymous incident intake,
case coordination, controlled alerts, and multi-channel delivery. The first operational
use case is missing children.

Submitted to the OSF/Andela hackathon under **Track 3: Safety, Reporting & Protection** — see
[`HACKATHON_SUBMISSION.md`](HACKATHON_SUBMISSION.md) for the full written summary
(track fit, information sources, trust/accuracy approach, AI tool usage).

## The problem

A bystander who witnesses a missing child or another protection incident often has only two bad
options: call a number that may not answer, or say nothing because reporting feels risky, slow,
or anonymous only in theory. On the other side, the organizations best placed to help (police,
NGOs, schools) have no reliable, targeted way to hear about incidents in their own area of
responsibility without being flooded by everything everywhere.

## How it works

```text
Citizen reports anonymously (no account)
  -> AI suggests structured fields from the free text (a suggestion, never a fact)
  -> reviewer verifies and opens a case
  -> reviewer issues a policy-authorized alert at a specific visibility tier
  -> subscription matching finds every consumer (org or citizen) who should hear about it
  -> multi-channel delivery, with fallback tiers gated on the primary channel actually failing
  -> every step is audited and attributable to the actor who caused it
```

## Why it's different

- **AI never gets the final word.** Extraction output is fully logged with provenance (model,
  prompt version, requester) but cannot create a case, verify an incident, or issue an alert by
  itself — a human always does.
- **Alert visibility is trust-granted, not self-declared.** An organization can only issue the
  alert visibility tiers the platform has explicitly granted it, so a community-facing alert can
  never accidentally carry internal-only detail.
- **The engine is generic; the vertical is the proof.** Incident types, geography matching, and
  delivery strategy are all domain-modeled to extend beyond one country or one incident class —
  missing children is the first hard case chosen to prove it end-to-end, not the ceiling of what
  the platform can do.

## Multi-country deployment model

Cameroon is the pilot, not a ceiling: each country gets its **own deployment** (own database, own
reviewers/organizations, own secrets), not a shared multi-tenant instance. Given this handles
child-safety data, police/NGO authority and legal reporting obligations are inherently national --
keeping deployments fully separate is a far simpler and stronger privacy/jurisdiction guarantee
than trusting a `country` filter on every query in a shared database, where one missed `WHERE`
clause would be a cross-country data leak. Concretely, a second country's deployment only needs to
set two things differently, both already config-driven rather than hardcoded:

- **`VITE_BRAND_NAME`** (`apps/console/.env.example`, `apps/citizen/.env.example`) -- the product
  name shown throughout both apps (headers, page titles, the citizen app's PWA manifest, footer
  copyright). Falls back to `"Sentinel"` if unset, so the existing pilot deployment needs no
  new configuration.
- **`apps/citizen/src/domain/knownTowns.ts`** -- the autocomplete suggestions for the citizen app's
  Area field. Not a validation allow-list (geography matching is free-text substring matching), so
  a different country's deployment just ships this one file with that country's town list.

Everything else -- the domain model, the API, the delivery/subscription engine, the CI/CD pipeline
-- is already country-agnostic and needs no code change per country.

## Repository layout

- `crates/domain`: pure domain types and state-transition rules.
- `crates/application`: use cases; infrastructure ports belong here as implementation begins.
- `apps/api`: Axum HTTP API.
- `apps/worker`: asynchronous worker that polls and dispatches deliveries.
- `apps/console`: TypeScript/React organization console (reviewer/admin, Google-authenticated).
- `apps/citizen`: public, offline-capable Preact app for anonymously reporting an incident (missing child or another protection incident) and self-subscribing to alerts (no login).

## Deployment

All four services (`api`, `worker`, `console`, `citizen`) run on Railway,
project `82278915-ac8b-4646-8826-358dd65b7d4f`, `production` environment.
`.github/workflows/ci.yml`'s `deploy` job redeploys all four automatically on
every push to `main` (i.e. every merged PR), after the `rust`/`console`/
`citizen` test jobs pass -- it never runs for a pull request, only for a
push directly to `main`. It authenticates as a project-scoped `RAILWAY_TOKEN`
repository secret; Railway only lets a token be minted through its dashboard
(Project Settings -> Tokens), not the CLI or API, so that secret has to be
created and added by a human with dashboard access, not by an agent.

Auto-deploy ships the already-built artifacts only -- it does **not** run
database migrations. A migration lands in `migrations/` in the same PR as
the code that needs it, but still has to be applied to production by hand
once, before (or as part of) that merge:
`railway run --service api --environment production --project <id> -- sqlx migrate run --source migrations`.

## Multi-area subscriptions

`SubscriptionRule::Geography` (`crates/domain/src/subscription.rs`) holds one
or more areas and matches if an alert's target intersects *any* of them (an
OR within the rule, the same semantics `IncidentType`/`EventType` already
had) -- a citizen or organization can subscribe to several towns/regions in
one subscription instead of needing a separate subscription per area. Stored
as a JSONB array (`crates/infrastructure/src/postgres/subscriptions.rs`);
reading still accepts the older single-string shape from subscriptions
persisted before this change, so no backfill migration was needed. Both
`apps/citizen`'s Area field and `apps/console`'s Geography rule editor are
typeahead multiselects (chips + a filtered dropdown, plus free text on the
citizen side for towns not in `apps/citizen/src/domain/cameroonTowns.ts`).

## Delivery fallback tiers

`DeliveryPreference` with `PrimaryFallback`/`PriorityList` numbers its channels
into tiers (0 = primary, 1 = first fallback, ...). `claim_next`
(`crates/infrastructure/src/postgres/deliveries.rs`) only dequeues a tier > 0
delivery once every lower-tier delivery for that same alert/consumer has
reached `FAILED_PERMANENTLY` -- a still-pending or already-succeeded lower
tier leaves the fallback un-claimable, so a working primary channel never
also sends the fallback message. Gating a delivery this way needs no extra
status: it simply stays `QUEUED` until the gate opens.

## Internationalization

`apps/citizen` and `apps/console` both support English and French. Language is
detected once per load from the browser's own language (`navigator.languages`),
falling back to English for any other language or when running outside a
browser (tests). There is no manual language switcher -- the browser's setting
is authoritative. Each app owns its own `src/i18n/` (`locale.ts` detection,
`translations.ts` a fully-typed `{ en, fr }` dictionary, `LanguageContext.tsx`
a context/hook pair) rather than sharing one, matching how each app already
duplicates its other small UI primitives (`Footer`, shield icon) instead of
introducing a shared package for a two-app monorepo. Adding a UI string means
adding the key to both languages in that app's `translations.ts` -- the
`Translations` interface makes a missing key or locale a compile error.

## SEO and discoverability (citizen app only)

The citizen app is meant to be found and shared -- it gets full search/AI-crawler
treatment: Open Graph and Twitter card meta tags with a real 1200x630 preview
image (link-preview quality matters here since this gets shared via WhatsApp
during real incidents), JSON-LD structured data (`WebApplication`), a canonical
URL, `robots.txt`/`sitemap.xml`, and an `llms.txt` (an emerging convention some
AI tools read for a plain-language site summary). `<html lang>` is set at
runtime to the detected locale (`apps/citizen/src/i18n/LanguageContext.tsx`),
since the static HTML shell has no server-side locale detection to set it
upfront. The console/portal gets the opposite treatment on purpose --
`<meta name="robots" content="noindex, nofollow">` and a blanket-disallow
`robots.txt` -- it's an authenticated org login with no benefit to public
search discovery.

## Pilot demo

`cargo run -p safe-cameroon-api --bin demo` (needs `DATABASE_URL`; `OPENROUTER_API_KEY`
optional) runs a real, narrated, end-to-end scenario against a real database and the
real mock/sandbox channel adapters (prompts/15_PILOT_DEMO.md): an anonymous report, AI
extraction, case review/verification, a community alert, four differently-configured
consumers (police/NGO/association/citizen) matching that alert differently, dispatch
across WhatsApp/SMS/email/push including a real provider validation failure, case
resolution with a follow-up alert, and the resulting audit trail. It calls the same
application-layer functions the HTTP handlers call, not mocks -- safe to run repeatedly
against a scratch/demo database (not idempotent; each run adds fresh rows).

## Local checks

Copy `.env.example` to `.env` and fill in real values, or prefix each
command with the vars inline as shown below — both binaries load `.env` at
startup via `dotenvy::dotenv()` if one is present, and otherwise read
straight from the process environment (a real deployment sets vars
directly and has no `.env` at all; a missing file is not an error).

```bash
cargo test --workspace
DATABASE_URL=postgres://... WEBHOOK_SHARED_SECRET=... ATTACHMENT_STORAGE_SECRET=... REVIEWER_SESSION_SECRET=... GOOGLE_OAUTH_CLIENT_ID=... CORS_ALLOWED_ORIGINS=http://localhost:5173 cargo run -p safe-cameroon-api
curl http://localhost:3000/health
DATABASE_URL=postgres://... VAPID_PRIVATE_KEY_BASE64=... VAPID_SUBJECT=mailto:you@example.com cargo run -p safe-cameroon-worker
```

`CORS_ALLOWED_ORIGINS` is a comma-separated allow-list of exact browser
origins (e.g. the deployed console's URL) permitted to call the API from a
browser; unset or empty allows none, since the API and console are always
served from different origins (docs/DEPLOYMENT.md).

`POST /v1/reports/{id}/extractions` (reviewer-only) asks an AI provider
(OpenRouter, `crates/infrastructure/src/ai/openrouter.rs`) to suggest
structured candidate fields (description, age, time, place, ...) from a
report's raw text -- always an unverified suggestion a reviewer reads in
the console, never applied to a report or case automatically (docs/AI.md,
CLAUDE.md "AI integration"). Optional: leave `OPENROUTER_API_KEY` unset to
disable the feature (the endpoint then fails clearly rather than the API
refusing to boot), since what data may go to an external AI provider is
itself an open question (docs/OPEN_QUESTIONS.md).

The worker polls for `QUEUED`/`RETRYING` deliveries and dispatches them
through mock/sandbox `WhatsAppChannel`/`SmsChannel` adapters (prompt 08;
stdout only, no vendor SDK) until real provider credentials exist, plus two
real channels: `WebPushChannel` (VAPID) -- the only channel a citizen can
self-subscribe to via `apps/citizen`, since a browser's own push
subscription is itself proof of ownership, unlike a phone number
(docs/OPEN_QUESTIONS.md) -- and `ResendEmailChannel`, used automatically
once `RESEND_API_KEY`/`RESEND_FROM_ADDRESS` are set (falls back to the
mock/sandbox `EmailChannel` otherwise). The API exposes `POST /v1/webhooks/{channel}/{provider}`
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
allowlists an email against a role/organization; it's open, unauthenticated
only to bootstrap a fresh deployment's very first reviewer (who always
becomes `PlatformAdmin`), and every registration after that requires an
authenticated reviewer whose own role permits the grant being requested
(see organizations, below). `POST /v1/auth/logout` revokes every session
currently issued to the calling reviewer ("logout everywhere") — a
compromised or offboarded reviewer no longer has to wait out a token's 12h
expiry.

Organizations and reviewer roles (`crates/domain/src/organization.rs`,
`POST/GET /v1/organizations`, `PUT /v1/organizations/{id}/trust`, `GET
/v1/organizations/{id}/members`) replace the flat "any identified reviewer
holds every capability" model docs/OPEN_QUESTIONS.md flagged as unresolved.
`PlatformAdmin` is org-independent and may register anyone into any
organization, verify any case, and issue any alert; `OrgAdmin` may register
only a `Member` into their own organization; a plain `Member` cannot
register anyone. Two of that section's three open governance questions —
"which authority can verify a case for each incident class" and "which
organizations may issue community/public alerts" — are answered by making
verification/alert-issuance authority an explicit trust grant on each real
`Organization` record (`verified_incident_types`,
`verified_alert_visibilities`), set by a `PlatformAdmin` when the
organization is onboarded, rather than a policy this codebase bakes in:
`POST /v1/cases/{id}/verify` and `POST /v1/cases/{id}/alerts` (for
community/public visibility) both check the caller's organization against
these grants, returning `403 ORGANIZATION_NOT_TRUSTED_FOR_INCIDENT_TYPE` /
`403 ORGANIZATION_NOT_TRUSTED_FOR_ALERT_VISIBILITY` otherwise —
`PlatformAdmin` bypasses both checks. The third open question, case
ownership across multiple organizations, stays open: case visibility/review
remains flat, any identified reviewer, since real multi-org case
routing/locking is a separate, larger workflow feature. Every reviewer who
existed before migration 0021 was grandfathered in as `PlatformAdmin`, so
nobody already provisioned lost access when it applied.

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

`POST /v1/reports` accepts an optional `incident_type` from the reporter
themselves (the citizen app's intake form asks "What kind of report is
this?"). It is never authoritative -- a reviewer still explicitly chooses
the case's `IncidentType` when creating a case from the report -- but it is
surfaced back through `GET /v1/reports`/`GET /v1/reports/{id}` as
`reported_incident_type` so the console can pre-fill that choice and show
it as a hint (`crates/domain/src/report.rs`).

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
