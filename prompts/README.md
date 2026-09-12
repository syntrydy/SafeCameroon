# Implementation Prompts

These prompts are designed for Claude Code, Codex, Gemini CLI, or another capable coding agent.

Recommended sequence:

```text
00_BOOTSTRAP
01_DOMAIN
02_DATABASE
03_REPORTING
04_CASE_WORKFLOW
05_ALERT_POLICY
06_SUBSCRIPTION_ENGINE
07_DELIVERY_ENGINE
08_CHANNELS
09_AUTH_SECURITY
10_ORG_CONSOLE
11_CITIZEN_EXPERIENCE
12_OBSERVABILITY_TESTING
13_DEPLOYMENT
14_HARDENING
15_PILOT_DEMO
```

Before each prompt:

1. Read `AGENTS.md`.
2. Read the relevant human spec.
3. Inspect current repository state.
4. Implement only the requested milestone.
5. Run tests/checks.
6. Update documentation if needed.
