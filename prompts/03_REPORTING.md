# Prompt 03 - Reporting

Read `docs/MVP_FLOWS.md`, `docs/API.md`, `docs/SECURITY_PRIVACY.md`.

Implement anonymous report intake.

Requirements:
- no login required;
- reference code returned;
- raw content preserved;
- identity_mode supports anonymous/private/identified;
- attachments are represented through a storage port and R2 adapter placeholder;
- follow-up by reference code;
- rate limiting hook/port;
- audit event for sensitive administrative actions, but do not over-audit the anonymous user's raw text.

Do not automatically create a verified case or public alert.

Add API and integration tests.
