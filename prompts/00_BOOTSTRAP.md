# Prompt 00 - Bootstrap

Read `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `docs/PRD.md`, and `docs/ARCHITECTURE.md`.

Create the initial repository skeleton for a Rust-first modular monolith plus worker architecture.

Requirements:
- Rust workspace.
- Shared modules/crates for domain, application, infrastructure.
- Axum API binary.
- Worker binary.
- Configuration module.
- Structured logging/tracing.
- Health/readiness endpoints.
- SQLx/Postgres configuration placeholders.
- React/TypeScript console skeleton if UI is included in this repo.
- `.env.example` without secrets.
- CI commands documented.

Do not implement business workflows yet.

Acceptance:
- repository builds;
- API starts;
- worker starts;
- health endpoint works;
- tests run;
- architecture boundaries are visible.
