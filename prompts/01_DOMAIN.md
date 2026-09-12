# Prompt 01 - Domain Model

Read `docs/DOMAIN_MODEL.md`, `docs/ALERT_SAFETY.md`, and `AGENTS.md`.

Implement strongly typed domain models for:
- Report
- Reporter
- Case
- CaseEvent
- Alert
- Consumer
- Subscription
- Delivery
- supporting enums/value objects

Use newtype IDs around UUIDs.

Implement explicit state-transition methods with errors for invalid transitions.

Do not add database code yet.

Write thorough unit tests for:
- valid transitions;
- invalid transitions;
- severity semantics;
- visibility semantics.
