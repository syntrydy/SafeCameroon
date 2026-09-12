# AGENTS.md - Repository Working Agreement

## Mission

Build a privacy-first civic protection platform for incident intake, case coordination, targeted alerts, and multi-channel delivery. The first operational use case is missing children; the platform must remain extensible to femicide, gender-based violence, abduction, child exploitation, and related protection incidents.

## Engineering defaults

- Use Rust for backend/application services and workers.
- Use Axum + Tokio for HTTP/async Rust services unless a documented reason says otherwise.
- Use SQLx with PostgreSQL (Neon initially).
- Prefer PostGIS for geographic matching once spatial behavior is implemented.
- Use Cloudflare R2 for large object storage.
- Use Cloudflare at the edge for WAF/rate limiting/routing where appropriate.
- Use PostgreSQL outbox + workers initially rather than premature Kafka/Kubernetes.
- Use TypeScript/React for the web console.

## Domain rules

- `Report` is raw incoming information.
- `Case` is a canonical incident under a workflow.
- `Alert` is a controlled projection of a case for a defined audience.
- `Subscription` decides which alerts a consumer wants.
- `Delivery` records an attempt to deliver an alert through a specific endpoint/channel.
- `Channel` is a transport abstraction; `Provider` is a concrete vendor/integration behind that channel.
- Reporter identity is optional for reporting and must never be assumed necessary for the existence of a report.

## Safety rules

- Never expose reporter identity by default.
- Never publish unverified allegations as verified facts.
- Never let an LLM directly authorize public dissemination of a sensitive allegation.
- Preserve original report content; AI output is an additional interpretation.
- Design for least privilege and field-level data minimization.
- Public/community alerts must be generated from explicit policy and visibility controls.
- Every sensitive read/write/action must be auditable.

## Coding rules

- Favor small modules with explicit boundaries.
- Prefer domain types and newtypes over generic strings/UUIDs everywhere.
- Keep domain logic independent of Cloudflare/Neon/provider SDKs.
- Use traits/ports at infrastructure boundaries.
- Use idempotency keys for externally visible operations.
- Make state transitions explicit and testable.
- Avoid hidden background side effects in request handlers.
- Errors must be typed and observable; do not swallow provider failures.
- Add unit tests for domain rules and integration tests for Postgres behavior.

## Database rules

- PostgreSQL is the system of record.
- Large media goes to R2; Postgres stores metadata/object keys.
- Use migrations and keep them reproducible.
- Prefer immutable event/audit rows for history.
- Do not rely on database-specific behavior unless it is documented in `docs/DATABASE.md`.

## AI rules

AI may:
- extract structured fields;
- translate/summarize;
- suggest incident classification;
- suggest information gaps;
- find duplicate/correlated reports;
- prioritize review.

AI may not:
- declare guilt;
- bypass authorization;
- directly send public alerts;
- rewrite or delete the raw source report;
- become the sole verification mechanism for a high-impact incident.

## Pull request expectations

Every meaningful change should state:

1. What domain behavior changed.
2. What security/privacy implications exist.
3. What tests were added/updated.
4. Whether any data migration is required.
5. Whether alerting behavior changed.
6. Whether an audit event is required.

## Definition of done

A feature is not done merely because the happy path works. It needs:

- tests;
- error handling;
- authorization checks;
- audit behavior when relevant;
- idempotency where relevant;
- observability;
- documentation update;
- migration rollback/forward strategy when relevant.
