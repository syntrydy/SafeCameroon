# SafeCameroon

SafeCameroon is a privacy-first civic-protection platform for anonymous incident intake,
case coordination, controlled alerts, and multi-channel delivery. The first operational
use case is missing children.

## Repository layout

- `crates/domain`: pure domain types and state-transition rules.
- `crates/application`: use cases; infrastructure ports belong here as implementation begins.
- `apps/api`: Axum HTTP API.
- `apps/worker`: asynchronous worker entry point.
- `apps/console`: future TypeScript/React organization console scaffold.

## Local checks

```bash
cargo test --workspace
DATABASE_URL=postgres://... cargo run -p safe-cameroon-api
curl http://localhost:3000/health
```

`docs/` contains local planning material and is intentionally excluded from Git.
