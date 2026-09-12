# QWEN.md - Qwen Code Instructions

Use `AGENTS.md` as the repository-wide contract. Read the relevant `docs/` specification and `prompts/` implementation prompt before editing code.

## Priorities

1. Preserve privacy and safety constraints.
2. Preserve domain boundaries.
3. Prefer Rust for backend/services/workers.
4. Keep infrastructure adapters replaceable.
5. Add tests with every meaningful behavioral change.

## Implementation expectations

- Inspect existing code before modifying it.
- Do not rewrite unrelated files.
- Do not introduce a new service/database/queue without a concrete requirement.
- Keep Cloudflare/Neon/provider-specific code out of the domain layer.
- Treat AI outputs as untrusted suggestions requiring validation and application-layer decisions.
- Never make anonymous reporting depend on authentication.

## Verification

Before completing a task, run the most relevant formatting, lint, unit, integration, and API tests and report failures honestly.
