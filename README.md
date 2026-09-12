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
DATABASE_URL=postgres://... cargo run -p safe-cameroon-api
curl http://localhost:3000/health
DATABASE_URL=postgres://... cargo run -p safe-cameroon-worker
```

The worker polls for `QUEUED`/`RETRYING` deliveries and dispatches them
through a `LoggingChannel` (stdout only) for every channel type; prompt 08
replaces this with real mock/sandbox WhatsApp/SMS/Email adapters behind the
same `Channel` port.

`docs/` contains local planning material and is intentionally excluded from Git.

Database integration tests are intentionally ignored by default. Run them only
against a dedicated PostgreSQL database whose name contains `test`:

```bash
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test report_submission_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test case_workflow_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test alert_policy_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-infrastructure --test delivery_engine_postgres -- --ignored
TEST_DATABASE_URL=postgres://... cargo test -p safe-cameroon-worker --bins -- --ignored
```

Run all integration test binaries with `--test-threads=1` if you see spurious
`TRUNCATE` deadlocks; each test truncates the shared schema, so they are not
safe to run concurrently against the same database.
