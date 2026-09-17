# OSF/Andela Hackathon Submission Summary

## Track

**Track 3: Safety, Reporting & Protection** — "platforms enabling anonymous reporting with
pathways to actual protection."

Sentinel (repository name `SafeCameroon`, after its pilot country) is an anonymous
incident-reporting and verified-alerting platform. The first
operational use case is missing children, chosen because it forces the hardest version of the
problem: a false negative can cost a life, a false positive erodes trust in every future alert,
and the reporter is often a stranger with no standing relationship to the platform. The
underlying engine (report intake, human verification, subscription-based alert matching,
multi-channel delivery) is not specific to that use case — `IncidentType` is an extensible enum,
and a second incident class (`OtherProtectionIncident`) already exists to prove the model
generalizes, not just describes an aspiration.

## Information sources

Every fact that reaches a reviewer, a case, or an alert has one of three origins, and the system
keeps them distinguishable rather than collapsing them into one "the data says" narrative:

1. **The reporter's own words** — an anonymous free-text report is the ground truth. Nothing is
   inferred on top of it without being labeled as such.
2. **AI-suggested structured fields** — an optional extraction step (person description, age,
   time, place, incident category, vehicle details, contact request) turns free text into
   structured candidates a reviewer can act on faster. These are suggestions, never facts, until
   a human accepts them (see below).
3. **A verified case** — only a reviewer's explicit case creation and status verification turns a
   report into something an alert can be built from. An alert always cites the case it came from
   and the policy that authorized it (`AlertPolicy`), so "why did this alert go out" always has a
   traceable answer.

## Trust and accuracy approach

- **AI is an untrusted recommendation service, not a decision-maker.** Every extraction call is
  stored with full provenance — provider, exact model, prompt version, who requested it, and the
  structured output itself (`ExtractionRecord` in `crates/application/src/ai_extraction.rs`) —
  and a reviewer decides whether to use any of it. No AI output can create a case, verify an
  incident, or issue an alert on its own; the system enforces this in code, not just in policy.
- **Tiered alert visibility bounds what any given recipient learns.** An alert's visibility
  (`INTERNAL` / `PARTNER` / `COMMUNITY` / `PUBLIC`) is set per alert against what the issuing
  organization is explicitly trust-granted to issue — a community Facebook-style alert never
  carries the same detail as what police receive internally, and an organization can't issue a
  visibility tier the platform hasn't granted it.
- **Every state change is audited and every delivery is idempotent.** Case transitions, alert
  issuance, and delivery attempts each write an audit/event record tied to the actor who caused
  them (`Actor::Reviewer(id)` or `Actor::Automated`), and delivery planning is keyed so replaying
  the same event twice (e.g. an at-least-once outbox consumer) never double-sends an alert.
- **Reporting itself stays anonymous by design.** A citizen report requires no account; only a
  privacy-preserving management token — never an identity — lets a citizen later manage their own
  subscription.

## AI tool usage

AI was used in two distinct ways, and this submission is honest that they're different:

1. **As the primary development partner.** The domain model, application use cases, HTTP API,
   Postgres persistence, both frontend apps, CI/CD, and the test suites were built through
   extensive iterative sessions with an AI coding agent (Claude Code) — reading the project's own
   specs one capability at a time, writing code and tests together, and catching real bugs along
   the way (e.g. a click handler that never fired because `role="option"` sat on the wrong DOM
   element; a delivery-fallback tier that was silently unguarded despite its own doc comment
   describing the intended gating). The product idea, scope, and every non-obvious design
   decision came from the author directing those sessions — AI assisted the build, not the
   concept.
2. **As a runtime feature.** The report-extraction pipeline calls an LLM (via OpenRouter) to turn
   a citizen's free-text report into structured candidate fields for a reviewer. This is the only
   place AI output reaches a human in the actual product, and it's deliberately the smallest,
   most constrained AI surface possible: one well-defined transformation, fully logged, never
   authoritative on its own.

## Scope honesty

Internal design notes describe a broader set of candidate AI tasks (information-gap detection,
duplicate-report correlation, summarization, AI-assisted translation) as future design space.
**Only extraction is implemented today.** Translation in the product today is static, professionally
written UI copy (English/French) selected by the browser's own language setting — not an AI
translation feature. This document intentionally does not claim more than what a judge can
actually run and verify.
