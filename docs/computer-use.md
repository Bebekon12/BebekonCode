# Local computer use (design, 0.8.x)

Status: **not implemented**. Approved by the owner on 2026-10-09. This document fixes the design
before code; the UI must keep showing computer use as unavailable until phase 2 ships.

## Why a local tool

Built-in Codex computer use and hosted tools are unavailable on the ChatGPT-plan route
([preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations/)).
Local MCP servers over stdio are supported ([MCP](https://learn.chatgpt.com/docs/extend/mcp)).
BebekonCode therefore ships its own MCP server; the model only proposes actions.

## Components

1. `crates/computer-use` — a separate binary `bebekon-computer`, MCP over stdio. Tools:
   `screenshot` (optional region, downscaled PNG), `click`, `double_click`, `move`, `drag`,
   `scroll`, `type_text`, `key` (named combinations), `wait`. Windows input uses `SendInput`;
   capture uses the documented Windows graphics capture/GDI APIs. No other OS in this phase.
2. Approval bridge — the binary never acts on its own. Each call is sent to the running core over a
   local-only named pipe (same model as the Claude hook bridge), with a per-run secret.
   The core emits `approval_requested` with kind `computer_use`, the action, coordinates and a
   thumbnail of the target area. Missing reply, timeout (60 s), cancellation or a closed app deny.
3. Core — enables the server per chat only when the user turns on «Управление компьютером»:
   thread-local Codex `config` adds `mcp_servers.bebekon_computer` (command = bundled binary path).
   The account `config.toml` is never rewritten. Read-only, review and auto workers never get it.
4. UI — a red bar «Агент управляет компьютером» with «Стоп» while a run with the tool is active;
   approval cards show the screenshot area and exact text to be typed. Stop cancels the run and
   releases held keys/buttons.

## Rules (from AGENTS.md)

- Every action, including each `screenshot`, needs a separate user decision in every access
  mode, including full access. Session-wide or repeated-action grants are not allowed.
- Screenshots go to the model provider as tool output; the toggle states this. They are not stored
  in SQLite or logs beyond the visible approval card of the current run.
- No credential capture: the tool refuses to type into the BebekonCode window itself and
  shows typed text in full before approval. Secrets are the user's responsibility to avoid.
- Claude: the adapter blocks `mcp__*` today. Support requires passing a single `--mcp-config`
  with this server and allowing only `mcp__bebekon_computer__*` through the existing hook, which
  forwards to the same approval bridge. Until then Claude shows the feature as unavailable.

## Phases

1. MCP server skeleton with `screenshot` only, approval bridge, per-chat toggle, indicator, tests
   with a fake desktop backend (no real input in CI).
2. Pointer and keyboard tools, stop/release handling, timeout and denial tests.
3. Claude support through the hook bridge.
4. Manual QA on a real Windows session: approvals, stop, multi-monitor and DPI scaling.
