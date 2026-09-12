# CLAUDE.md - Claude Code Instructions

Read `AGENTS.md` first. Then read the relevant document under `docs/` before implementing a feature.

## Default workflow

1. Identify the domain capability being changed.
2. Read the corresponding spec and prompt in `prompts/`.
3. State assumptions in code comments or PR notes when they are not obvious.
4. Implement domain logic before adapters.
5. Add or update tests before moving to integration code.
6. Run formatting, linting, unit tests, database/integration tests, and relevant API tests.
7. Update documentation when behavior changes.

## Rust preferences

- Stable Rust edition defined by the project configuration.
- Axum for HTTP.
- Tokio for async runtime.
- SQLx for PostgreSQL.
- Serde for API/event serialization.
- `thiserror` for library/domain errors; `anyhow` only at appropriate application boundaries.
- `tracing` for structured logging.
- Newtype IDs for domain identifiers.
- Avoid `String` for stateful enums such as incident type, severity, status, and channel wherever a Rust enum is appropriate.

## Implementation style

Prefer a clean architecture with:

```text
Domain -> Application -> Ports -> Infrastructure
```

The domain must not import Cloudflare, Neon SDKs, HTTP provider SDKs, or UI concerns.

## Important safety behavior

Before adding an alert path, confirm:

- verification state is sufficient;
- alert visibility is explicit;
- sensitive fields are not leaked;
- recipients are authorized/matched;
- delivery is idempotent;
- actions are auditable.

## AI integration

Treat AI as an untrusted recommendation service. Require structured output, validation, provenance, model metadata, and an explicit application-layer decision before changing case state or producing an alert.

## Do not overengineer the first release

Prefer one modular Rust application plus dedicated worker binaries/processes. Extract services only when scaling, isolation, ownership, or security boundaries justify it.
