# Prompt 09 - Authorization and Security

Read `SECURITY.md`, `docs/SECURITY_PRIVACY.md`, and `AGENTS.md`.

Implement a policy-oriented authorization layer.

Authorization must consider:
- actor;
- action;
- resource;
- organization;
- role;
- geographic/scope context;
- visibility.

Add audit events for sensitive operations.

Implement protected attachment access through a storage abstraction and short-lived authorization.

Do not require authentication for anonymous reporting.
