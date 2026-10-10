# Claude usage audit — 2026-10-10

The app can consume more allowance on a task than a single Claude Desktop chat, especially
in team/auto mode. No live, matched Desktop comparison has been performed; neither a 2–5×
multiplier nor a broken prompt cache is established by this audit.

## Observed integration

Inference uses the official native Claude Code CLI, `--print` and `stream-json`.
The saved upstream session ID is continued with `--resume` within the explicitly bound
account profile. A single resumed chat receives the new message and its attachments,
not the entire app transcript. Handoff injects bounded local history into a fresh target
thread once; handoff itself no longer calls the exhausted source model.

Team orchestration schedules additional inference. These are adapter launches, not counts
of underlying model requests: each launch may contain multiple tool/model round trips.

| Mode                                    | App-scheduled launches for one message   |
| --------------------------------------- | ---------------------------------------- |
| Single chat                             | 1                                        |
| Team with one worker and a coordinator  | 2: worker and coordinator                |
| Team with two workers and a coordinator | 5: two workers, two reviews, coordinator |
| Auto with 1–4 planned tasks             | 3–6: planner, tasks, coordinator         |

Counts assume successful runs, exclude provider-internal calls and any separately requested
chats. Existing configured team members are templates: task sessions start fresh for each
message and continue their own thread for the read-only review. The coordinator resumes
its thread. Opus, effort, large files/MCP results, and tool-heavy loops can further change
usage. Refreshing model catalogs or the documented local zero-inference usage command is
not an additional inference summary stage.

## Corrections made

- A fresh worker previously received shared history both inline and through
  `context_summary`. It now receives that history once.
- A resumed coordinator previously received the full bounded local history again,
  despite continuing its upstream thread. Local history is now supplied only for a
  fresh coordinator thread; current task and worker results are still supplied.
- Team prompts previously repeated attachment descriptions during orchestration and
  worker execution. Each direct run now appends the description once.
- Consecutive streamed worker chunks no longer repeat the same worker header in the
  shared context. Public results remain preserved.

These remove concrete duplicate prompt data. Their effect on subscription allowance or
cache hit rate has not been measured. Shared context deliberately preserves the original
goal and latest user correction separately; it is not deduplicated by dropping instructions.

## Diagnostics

The local event log now records app-scheduled provider launches, purpose, requested model,
new/resumed status and prompt byte length. Claude result events contribute only bounded
numeric main-loop `usage` counters and `num_turns` when reported. No inference is added
to calculate diagnostics. The expandable **Расход Claude** view aggregates these for
each user turn and labels missing/partial data.

The pinned official SDK result schema specifies that `usage` covers the turn's main
agent loop only. It excludes some auxiliary/sidechain/subagent calls. `modelUsage` and
`total_cost_usd` can include prior resumed turns, so they are not summed or converted
into subscription percentages. Interrupted runs can lack final counters. Prompt byte
length is not a token count. Raw provider payloads and private thinking are not stored.

For a fair comparison, use the same fixed model, effort, task, tools/MCP, attachments and
access in new sessions, with single mode in this app. Compare reported launch/token/cache
data and official account Usage; do not equate a monetary cost estimate to plan allowance.

## Fast mode

Fast is independent of effort, disabled by default, and enabled per agent after a visible
consumption warning. Codex support follows the actual bound model catalog and uses a
turn-only tier override. Claude support is limited to documented fixed Opus IDs; aliases
and other models show unavailable. Claude Fast spends additional paid usage credits even
when subscription allowance remains. Quiet auto planning does not inherit paid Fast.
Provider entitlement, acceptance and acceleration still require live verification.

## Evidence and limits

Tests cover actual fixture CLI settings (`fastMode` true, then false on the same resumed
session), unchanged approvals/account binding, one result-usage event per launch, ignored
cumulative cost/usage fields, context/attachment deduplication, persisted defaults and
unsupported-model rejection. Browser tests cover independent effort, confirmation,
Claude's paid-credit warning, model support and a new chat starting with Fast off.
No live quota-consuming model turn or production release was performed.

Validation: 92 Rust tests, 41 frontend tests and six browser scenarios passed; five
provider-environment tests remain explicitly ignored. Formatting, Clippy, TypeScript,
frontend production build and workspace compilation including Tauri passed. A real
temporary Git repository verifies turn snapshots through the application lifecycle.
Snapshots apply to Git-listed tracked/untracked files; non-Git folders and ignored files
are not covered, and concurrent edits cannot be attributed exclusively to the agent.

Official sources checked:
[Claude headless](https://code.claude.com/docs/en/headless),
[Claude CLI](https://code.claude.com/docs/en/cli-reference),
[shared usage limits](https://support.claude.com/en/articles/11647753-how-do-usage-and-length-limits-work),
[Claude Fast](https://code.claude.com/docs/en/fast-mode),
[Codex speed](https://learn.chatgpt.com/docs/agent-configuration/speed),
[Codex app-server](https://learn.chatgpt.com/docs/app-server).
The installed Codex 0.160.1 stable generated JSON schema and pinned official Claude Agent
SDK 0.3.295 declarations were also inspected locally without inference or credential access.
