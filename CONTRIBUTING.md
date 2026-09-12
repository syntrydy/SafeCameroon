# Contributing

## Before coding

Read:

1. `AGENTS.md`
2. relevant `docs/*.md`
3. relevant `prompts/*.md`

## Branches/commits

Keep changes focused. Prefer commits that correspond to a coherent domain capability or migration.

## Pull requests

Include:

- summary;
- affected domain objects;
- database changes;
- security/privacy impact;
- test evidence;
- operational impact;
- documentation changes.

## Local checks

Typical Rust checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Use actual project commands as the source of truth once the repository is bootstrapped.

## Sensitive-data development

Never commit real victim/reporter data, production attachments, secrets, or real personal phone numbers/emails into tests. Use synthetic fixtures.
