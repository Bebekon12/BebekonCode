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
- **Sessions.** `thread/start` / `thread/resume` / `turn/start` / `turn/interrupt`. The Codex thread
  id is stored as the session's `provider_session_id`. A session's account never changes.
- **Events.** Agent-message deltas, commands, file changes, MCP calls, web search, plans and errors
  are normalized in `codex/mapping.rs`.
- **Approvals.** Command and file-change requests appear in the timeline with command, folder and
  reason: allow once, allow for the session, or deny. Other request types are declined visibly.
- **Models, plugins, MCP, skills.** `model/list`, `plugin/list`, `mcpServerStatus/list` and
  `skills/list` per account.
- **Usage.** Shown only when Codex reports it; otherwise the account links to ChatGPT usage
  settings. A usage-limit error stops work; switching accounts is a manual user action.
- **Process hygiene.** Allowlisted environment (no other provider's keys), no console window, each
  process tree in its own kill-on-close Job Object, pipes read on dedicated OS threads.

Tested with codex-cli 0.160.1. Live checks that start the real CLI without signing in:
`cargo test -p agent-core --test codex_live -- --ignored`.

## Anthropic / Claude Code (`anthropic`)

Detected only. Planned: the unmodified installed Claude Code, one `CLAUDE_CONFIG_DIR` per account,
sign-in through Claude Code's own flow, `claude -p` with `stream-json`. See
[provider compliance](provider-compliance.md) for the constraints that apply.
