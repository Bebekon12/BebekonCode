# Bundled Claude usage adapter

The embedded usage helper includes the official `@anthropic-ai/claude-agent-sdk` 0.3.295
JavaScript distribution, bundled with esbuild; the SDK's native Claude binary is not shipped.
The user-installed official Claude Code executable owns account authentication.

Anthropic's distribution license notice:

> © Anthropic PBC. All rights reserved. Use is subject to the Legal Agreements outlined here:
> https://code.claude.com/docs/en/legal-and-compliance.

The helper preserves bundled legal comments. The SDK is third-party software and is not
covered by this repository's MIT license. Dependencies and integrity hashes are recorded
in `package-lock.json`.
