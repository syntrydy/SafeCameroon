# Prompt 06 - Subscription Engine

Read `docs/SUBSCRIPTION_ENGINE.md`.

Implement deterministic subscription matching.

Support:
- incident type;
- severity comparisons;
- event type;
- geography abstraction (PostGIS-backed when enabled);
- match explanations;
- consumer-level deduplication;
- subscription version metadata.

Create a `MatchDecision` with reasons.

Do not use an LLM for recipient selection.

Create extensive unit tests for match/mismatch combinations and multiple subscriptions for one consumer.
