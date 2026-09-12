# Prompt 08 - Channels

Read `docs/CHANNELS.md`.

Implement channel/provider ports and adapters.

First adapters may be mock/sandbox implementations if external credentials are not available.

Required channel contracts:
- WhatsApp;
- SMS;
- Email.

Each provider adapter must encapsulate provider-specific payloads and errors.

Implement webhook verification/replay protection interfaces where callbacks exist.

Do not put vendor SDK types in domain/application modules.
