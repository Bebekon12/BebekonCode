# Providers

Providers implement `AgentProvider` (`crates/agent-core/src/provider.rs`). Session management,
storage and the UI never branch on a provider id; unsupported features stay visibly unavailable.

## Local demo (`mock`)

Deterministic simulator for development. It emits illustrative activity and text without reading
the project. Its built-in account needs no sign-in.

## OpenAI / Codex (`openai`)

Official `codex app-server` over stdio, one process per account (`crates/agent-core/src/codex`).

- **Accounts.** Each account has its own Sign in with ChatGPT registration (issued client id), a
  DPAPI-protected credential record and its own `CODEX_HOME`. Accounts never share tokens.
- **Sign-in.** "Continue with ChatGPT" opens OpenAI's authorization page; a one-shot listener on
  `127.0.0.1` receives the callback, then the code is exchanged and the ID token validated.
  For subscription quotas, an explicit "Sign in through Codex" action uses the official
  `account/login/start` ChatGPT flow instead, in the same isolated `CODEX_HOME`. Codex owns
  credentials in the OS keyring, refresh and logout; the selected mode has no automatic fallback.
- **Sessions.** `thread/start` / `thread/resume` / `turn/start` / `turn/interrupt`. The Codex thread
  id is stored as the session's `provider_session_id`. A session's account never changes.
- **Events.** Agent-message deltas, commands, file changes, MCP calls, web search, plans and errors
  are normalized in `codex/mapping.rs`.
- **Approvals.** Command and file-change requests appear in the timeline with command, folder and
  reason: allow once, allow for the session, or deny. Other request types are declined visibly.
- **Models, MCP, skills.** `model/list`, `mcpServerStatus/list` and `skills/list` per account.
- **Plugins.** The documented official CLI `plugin list --available --json`, `plugin add`
  and `plugin remove` use this account's own `CODEX_HOME`. Marketplace installation is blocked
  while the account is working and its app-server is restarted after changes. Unity's official
  marketplace can be connected explicitly. App-server `plugin/*` production methods are not used.
  Hooks are disabled; desktop/computer-use plugins remain unavailable. Default provider plugins
  cannot be removed here. External-service authorization and plugin compatibility are not implied
  by installation. Native Codex sign-in is needed for its authenticated remote catalog.
- **Usage.** Read with `account/rateLimits/read` without inference and updated by
  `account/rateLimits/updated`. Shown only when Codex reports it; otherwise the account links to ChatGPT usage
  settings. A usage-limit error stops work; switching accounts is a manual user action.
- **Process hygiene.** Allowlisted environment (no other provider's keys), no console window, each
  process tree in its own kill-on-close Job Object, pipes read on dedicated OS threads.

Tested with codex-cli 0.160.1. Live checks that start the real CLI without signing in:
`cargo test -p agent-core --test codex_live -- --ignored`.

## Anthropic / Claude Code (`anthropic`)

Unmodified installed native Windows Claude Code **2.1.293+**, one explicit `CLAUDE_CONFIG_DIR`
per account. Sign-in and credential storage belong exclusively to the official CLI. No copied
default-client credentials, embedded OAuth, provider keys in the frontend or custom endpoints.

- `auth status/login/logout`; the CLI opens its own browser flow. Console login instructions are
  displayed with the exact isolated profile. No quota percentages are inferred.
- Subscription quotas use the embedded, pinned official Agent SDK 0.3.295 public experimental
  usage method, without a prompt, tools or transcript scanning. Local Node.js 18+ is required.
  Reported five-hour, weekly and model windows include reset times. SDK/runtime/authorization
  failures remain visible. The SDK does not own inference or replace the installed CLI.
- `claude --print --output-format stream-json --verbose --include-partial-messages`, prompt over
  stdin, UUID session resume, structured output for automatic task decomposition.
- Sonnet/Opus/Haiku are documented aliases, not a discovered entitlement catalog. Anthropic
  enforces actual model/effort availability and billing. No configured fallback model or account.
- `--restricted`, default permissions, an explicit file-tool allowlist, empty strict MCP config,
  disabled custom slash commands and only managed/explicit settings. No Windows shell tools,
  web tools, plugins, skills or nested agents: the native Windows shell is not sandboxed.
- PreToolUse validates project paths; PermissionRequest forwards file-write approvals through a
  local-only named pipe and exec-form hooks. Cancellation, missing UI, hook errors and timeouts
  cannot grant a write. Session grants cover one canonical file and stay in memory.
- Only assistant text and tool activity are normalized; hidden reasoning is not displayed or
  persisted. Missing final results and limit/auth errors fail visibly. Job Objects stop owned
  process trees. No telemetry is added; documented CLI telemetry switches are disabled.

Core tests drive an explicit protocol fixture through real subprocess pipes and named-pipe
approvals, mixed teams, auto mode and handoff. They do not prove live subscription inference.
`cargo test -p agent-core --test claude_live -- --ignored` tests the actual installed CLI against
a fresh signed-out profile without inference or copying any credentials.
`cargo test -p agent-core installed_cli_accepts -- --ignored` checks that the actual CLI parses
the protected launch flags and returns an authentication error before inference. After packaging,
`node scripts/claude-native-audit.mjs` checks exec-form hook IPC in the GUI executable;
`npm run test:native -- --claude` checks real Tauri detection and an isolated signed-out profile.
See [provider compliance](provider-compliance.md) for official sources and limitations.
