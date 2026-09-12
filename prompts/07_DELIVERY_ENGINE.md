# Prompt 07 - Delivery Engine

Read `docs/CHANNELS.md` and `docs/EVENTS.md`.

Implement asynchronous delivery planning and execution.

Requirements:
- Delivery records;
- DeliveryAttempt records;
- idempotency;
- retryable vs permanent failures;
- primary/fallback and all-channel strategies;
- outbox/job processing;
- provider-neutral interfaces;
- delivery status events.

A provider failure must not fail alert creation.

Test duplicate jobs and concurrent workers.
