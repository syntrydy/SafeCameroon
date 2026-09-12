# CODEX.md - Codex Implementation Instructions

Use `AGENTS.md` as the repository contract and treat `docs/` as the product/architecture source of truth.

## Work style

- Inspect existing code before changing it.
- Make the smallest coherent implementation that satisfies the current milestone.
- Do not introduce infrastructure solely for architectural fashion.
- Keep interfaces explicit so future service extraction remains possible.

## Required checks

For Rust changes:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Use project-specific commands when they are more accurate.

For schema changes:

- add a migration;
- test forward migration;
- test application behavior against the migrated schema;
- document destructive changes;
- ensure indexes support subscription matching and audit queries.

## Safety-critical areas

Treat incident verification, alert policy, subscription matching, privacy projections, delivery idempotency, and authorization as high-risk code. Write decision-table tests for these areas.

## Output discipline

Do not invent provider capabilities. When integration behavior depends on a current vendor API, verify the current provider documentation and capture the relevant implementation assumption in the integration doc.
