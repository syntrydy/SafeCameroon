# Prompt 16 - Final Code Review

Read all current specs and inspect the repository as if reviewing a production pull request.

Check:
- domain boundaries;
- data leakage;
- authorization;
- migration quality;
- idempotency;
- race conditions;
- retry semantics;
- audit coverage;
- provider abstraction;
- subscription determinism;
- test quality;
- logging/privacy;
- operational readiness.

Return findings grouped by severity, then implement safe fixes. Do not add speculative infrastructure.
