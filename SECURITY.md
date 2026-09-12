# SECURITY.md

This project concerns vulnerable people and potentially life-changing information. Security is part of the product, not only an infrastructure concern.

## Threat model summary

Assume:

- malicious anonymous reports;
- impersonation of organizations;
- account takeover;
- insider curiosity/access;
- accidental disclosure through notification content;
- compromised messaging providers;
- replayed webhooks;
- duplicate/reordered events;
- stolen API credentials;
- abusive automation and spam;
- malicious attachments;
- attempts to deanonymize reporters;
- attempts to turn unverified accusations into public accusations.

## Security objectives

- Confidentiality of sensitive case and reporter data.
- Integrity of case state and alert history.
- Availability for critical reporting/delivery workflows.
- Accountability through immutable audit events.
- Privacy-preserving defaults.

## Required controls

- Authentication for protected organizational capabilities.
- No login requirement for anonymous reporting.
- Strong organization/user authorization.
- Short-lived signed access for private files.
- Encryption in transit and at rest through managed infrastructure.
- Secret management outside source control and ordinary database rows.
- Webhook signature verification and replay protection.
- Idempotency on report/alert/delivery commands.
- Rate limiting and abuse controls at the edge and application layer.
- Attachment validation and malware scanning strategy before broad distribution.
- Structured audit logs for sensitive actions.
- Data retention policies by data class.
- Incident response procedure.

## Reporter privacy

A reporter may choose anonymous, private, or identified interaction. A private reporter can be contacted without exposing their contact endpoint to every case consumer.

## Public alert safety

The system must have a clear separation between:

1. raw reports;
2. verified cases;
3. internal notes/evidence;
4. partner alerts;
5. community/public alerts.

Only explicitly permitted fields can cross those boundaries.
