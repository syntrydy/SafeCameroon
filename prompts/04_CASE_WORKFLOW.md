# Prompt 04 - Case Workflow

Read `docs/ALERT_SAFETY.md` and `docs/DOMAIN_MODEL.md`.

Implement:
- report-to-case creation;
- linking multiple reports;
- review status;
- verify/reject/cancel/resolve transitions;
- case events;
- reviewer authorization hooks;
- outbox event creation in the same DB transaction as the state transition.

AI suggestions may be stored but must not directly verify a case.

Add decision-table tests for all state transitions.
