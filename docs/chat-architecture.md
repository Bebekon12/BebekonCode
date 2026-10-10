# Chat execution

A user-visible chat keeps the original session ID and event history. `0002_chats.sql` adds
configuration, parent links, roles, modes and a handoff summary without rewriting migration 0001
or existing events. Old sessions become single chats and keep their provider thread IDs.

The selected provider/account is explicit. Model/effort/access/tool changes are permitted between
turns. A handoff reserves the chat and its children, validates the target, builds a bounded,
redacted context snapshot from local saved history (including public worker messages, attachments,
errors and partial work), then atomically stores the target binding and resets the upstream
thread ID. It makes no inference request to the source or target, so an exhausted source quota
cannot block transfer. Errors/cancellation before committing retain the source. The first target turn includes
the stored snapshot. Saved chat events are not deleted or rewritten; new replies record their
provider/model configuration. Provider-side auto-compaction remains the provider's responsibility.

An explicit handoff changes team/auto chats to single-agent execution. The dialog states this
before the user switches. Old child configurations and task histories remain stored but are never
scheduled by subsequent messages in that chat; no child is rebound or granted additional access.
Separately running children block transfer. Previous handoff activity snapshots are not recursively
quoted into new snapshots. This is a local history transfer, not a provider-generated verification
of work or a guarantee that an entire long history fits the bounded context.

Team participants are saved child configurations. Each user request creates separate child task
sessions, including a bounded common context; at most two run concurrently. Team review sends
the first results to the same child threads for one round of discussion. Auto planning uses a
fresh read-only coordinator thread with JSON Schema output and validates 1–4 independent tasks
and their participant indices. Each task gets its own upstream thread and events. Only the main
coordinator defaults to workspace-write. Permission requests remain user decisions.

The coordinator produces the final answer after successful workers. An error stops the wave;
the app never silently substitutes a different account or synthesizes fabricated success. Parent
cancellation propagates to child cancellation tokens. Reserved-session leases cancel children if
orchestration is dropped. Database reopening marks previously running sessions interrupted and
does not replay tool execution. Continuing a partially completed graph is not implemented.

History context includes the original user goal and up to 5,000 recent events. Text is bounded;
truncation is marked. Workflows have at most three configured participants, four independent
auto tasks and one review round. This is bounded collaboration, not unlimited nested delegation.

Tool IDs come from the account's official inventory. `null` inherits account availability; an
array selects only those installed items. Codex receives thread-local MCP/skill configuration and
per-turn disabled plugin IDs; no global configuration is edited. The current adapter supports
Codex CLI 0.160.1. Other providers must implement and validate these semantics before advertising
tool selection as available. Hosted image generation is unavailable on the ChatGPT-plan route.

Integration tests use deliberately labelled test engines to verify execution state, transfer,
account binding, next-turn settings, concurrent read-only tasks, review and cancellation. Native
tests use a separate app data directory. Live provider quality/quotas and Claude integration are
separate validation milestones listed in the Russian roadmap.
