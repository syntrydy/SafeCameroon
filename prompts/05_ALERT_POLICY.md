# Prompt 05 - Alert Policy

Read `docs/ALERT_SAFETY.md`, `docs/DOMAIN_MODEL.md`, and `docs/API.md`.

Implement an alert policy abstraction that can transform a verified case into an alert-safe projection.

Requirements:
- explicit policy ID/version;
- visibility level;
- target geography;
- field allowlist;
- event trigger;
- no internal notes/reporter identity in community/public output;
- deterministic validation.

Start with missing-child community alerts.

Add tests proving sensitive fields cannot leak into community/public alert projections.
