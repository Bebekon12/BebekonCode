# Codex usage audit — 2026-10-10

The app uses the official `codex app-server` over stdio, with an isolated server/home per
explicitly selected account. It already resumes the saved thread through `thread/resume`.
New single-chat turns append the current message and attachments; the entire application
history is not manually appended on every turn. No automatic new thread is created if
resume fails, and no other account is substituted.

## Concrete findings

Team and auto modes schedule additional work exactly as in the
[Claude audit](claude-usage-audit.md): a coordinator plus one worker takes two launches;
two workers with review and synthesis take five; auto planning plus 1–4 workers and a
coordinator takes 3–6. Each launch can contain many model/tool round trips. Single mode
does not schedule a separate model just to polish the final answer.

The context/attachment deduplication fixes apply to both providers. Fresh workers no
longer receive the same shared history twice. Resumed coordinators no longer receive
the full bounded local history in addition to their upstream thread. Each direct run
gets attachment descriptions once. This reduces actual prompt bytes, but savings in
subscription quota or cache hits have not been measured.

The adapter previously ignored `thread/tokenUsage/updated`. It now aggregates numeric
usage per launched user turn and displays **Расход Codex**: input, cached input, cache
creation where reported, output, and reasoning output. Codex input includes cached
input; reasoning is part of output. Neither is added to the containing total again.
MCP completion and attachment-image counts describe the whole message, across agents;
they are not a count of screenshots or hidden/internal model calls.

Thread `total` is cumulative; `last` is the latest model request. The adapter uses a
known pre-turn baseline, suppresses duplicate cumulative updates and emits one summary
at completion. Restored totals are numerical in-memory account-server data only.
Unknown baselines, malformed counters, resets/compaction and interrupted turns make
the result incomplete. In that case the available latest-request counters are reported
as partial observations rather than the entire old thread being charged to this message.
No percentages, money, private reasoning or raw tool results are inferred or persisted.

Late `turn/completed` events are checked against their nested turn ID as well as the
thread ID. A previous-turn completion is ignored while a newer answer is running.

Existing `account/rateLimits/read` and update notifications remain account-level
snapshots where supported by the selected auth mode. Parallel chats, coarse percentages
and delayed upstream reporting make them unsuitable for an exact per-task bill. The
audit adds no quota polling, summary call or inference stage. SIWC quota unavailability
continues to be shown explicitly rather than querying an incompatible or private API.

## Verification and limitations

An in-memory JSON-RPC fixture checks thread creation, resume of the same ID, current
input only, per-turn Fast/default overrides, unchanged approvals, multiple/duplicate
usage updates and a stale previous-turn completion. Aggregation tests cover known and
unknown baselines, malformed values, resets and fresh threads. UI tests distinguish
provider counters and mark incomplete data without fabricating quota percentages.
These are protocol tests, not a measured live Desktop/app comparison.

Model, effort, Fast, tools and task scope must match for a fair comparison. Large MCP
results and repeated file/image reads are driven by the task and model, not automatically
deduplicated by this app. A fixed 2–5× multiplier or broken provider prompt cache has not
been demonstrated. Fast is opt-in and is a higher-consumption service tier, not an
allowance-saving mode. Actual entitlement and acceleration require live verification.

Official sources checked:
[Codex app-server](https://learn.chatgpt.com/docs/app-server),
[Codex speed](https://learn.chatgpt.com/docs/agent-configuration/speed).
Installed official Codex CLI 0.160.1 stable schemas were checked without inference or
credential access, including `ThreadTokenUsageUpdatedNotification`, its breakdown,
model service tiers and `TurnStartParams.serviceTierForTurn`.
