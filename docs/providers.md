# Providers

## Available now

MockProvider is the only runnable provider. `mock-stream-v1` is a deterministic simulator,
not a hosted model. It emits illustrative planning activity and text without reading the project.
The local demo account requires no authentication. OpenAI/Anthropic are detected by executable
path but explicitly unavailable; detection is not connection or proof of version compatibility.

## Planned official adapters

OpenAI: app-server over stdio, initialize/initialized, thread/start/resume and turn/start.
Normalized agent-message deltas and terminal turn status must be translated in the adapter.
For independent subscription profiles, use documented Sign in with ChatGPT registration and
token lifecycle. Each profile has a stable UUID, registration and native credential reference.
Do not reuse a consumer client registration or import auth.json. `model/list` supplies a catalog,
not guaranteed entitlement. Quota failure never changes the session's account.

Anthropic: official installed Claude Code, programmatic stream-json or an officially supported
permission-capable host transport. Each profile gets a separate CLAUDE_CONFIG_DIR under app
data. Login is performed by Claude's official flow in that profile. Do not copy tokens or read
another application's credentials. Validate how configuration isolation covers auth for the
tested installed version; directory isolation alone is not a blanket credential security claim.

Before implementation, verify versioned protocol fixtures, account isolation, cancellation,
permissions, rate-limit events, environment filtering and Windows Job Object cleanup. No CLI
versions are claimed as tested for actual provider inference in 0.1.0.

See [provider compliance](provider-compliance.md) for official sources and review date.
