# Architecture and initial plan

Status: implemented first vertical slice, version 0.1.0. This is an early development release,
not the full production V1. The working name is Nexus; UI identity and repository live in
`product.json`, installer identity in `src-tauri/tauri.conf.json`. Keep the application identifier
stable when renaming so existing local data is retained.

```text
React / TypeScript
    ClientTransport (commands, queries, normalized events)
        Tauri IPC adapter
            agent-core (Rust / Tokio)
                Session runtime -> AgentProvider -> MockProvider
                Storage -> SQLite / WAL / migrations
                Permission policy + canonical path boundary
                Native credential storage interface
                Read-only Git runner
                GitHub release checker
```

The core crate does not import Tauri. In 0.1.0 it runs inside the desktop process; it is not a
separate background daemon. A UI reload keeps the core alive, while exiting the application
ends the core. A future headless host can instantiate the same Core and expose a second
ClientTransport without changing provider logic. No server or remote transport exists today.

## Implemented slice

1. Native Tauri 2 shell and a compact graphite design system; system fonts and Lucide icons.
2. Typed IPC and normalized events, projects, immutable provider/account bindings and sessions.
3. Persistent SQLite event history, batched UI streaming, pagination, per-session cancellation.
4. A deterministic MockProvider, visibly marked as simulation. It does not execute tools.
5. Provider executable detection without inspecting credentials or launching a login.
6. Permission policy, canonical containment checks, OS credential interface.
7. Settings, command palette, bounded read-only Git status and unified staged/unstaged diff.
8. Opt-in startup/manual GitHub release checks, SemVer comparison, changelog and release link.
9. Windows installer packaging and CI/release workflows.

## Significant library decisions

- Tokio: one runtime, tasks per turn, bounded mpsc (64) and broadcast (512), cancellation tokens.
- sqlx/SQLite: async persistence, checked-in migrations, WAL, foreign keys, transactions.
  SQLite is bundled; no database service is installed. Only SQLite is enabled at runtime.
- Tauri 2: system WebView2; Node is a frontend build tool. No Electron runtime.
- thiserror: typed core errors with safe user-facing messages; no catch-all anyhow layer.
- keyring with windows-native: Windows Credential Manager; no home-grown token encryption.
  The interface exists; no real OAuth login or refresh lifecycle is implemented yet.
- reqwest with rustls: GitHub HTTPS checks in Rust, bounded body and timeouts. No webview HTTP
  permission is needed. semver compares versions numerically and excludes preview tags.
- React primitives: no global state library, terminal emulator or heavyweight editor at startup.

## Durability and lifecycle

The event insert and session status transition commit in one transaction before IPC emission.
SQLite is authoritative; the event channel is a notification path. Subscribe before querying
history; merge by event sequence to deduplicate history/live races. Channel lag requests a
fresh snapshot and history. The UI batches events at 50 ms and bounds displayed history.
History is paged in 300-event windows; the database retains the complete event log.

One run owns one CancellationToken; the active-run registry rejects overlapping turns within
a session while allowing other sessions to progress. On core restart, running sessions become
interrupted; no prompt is replayed automatically. On a UI reload, live events and the persisted
timeline reconnect. A second desktop instance focuses the first instead of reopening its DB.

## Next milestones

1. Unified ProcessManager with Windows Job Objects, bounded protocol readers, clean environment,
   graceful stop, crash events and real protocol fixture tests.
2. Claude Code adapter with per-profile CLAUDE_CONFIG_DIR and official login; preserve permissions.
3. OpenAI app-server adapter and documented Sign in with ChatGPT per-profile registration,
   native token storage, refresh and thread resume. Dynamic model catalog, no entitlement guesses.
4. Native approval routing, worktree creation and review workflow; never automatic merge.
5. Capability router with explicit bindings, audit records, context minimization and delegation caps.
6. Signed Tauri automatic updates after a persistent signing key is provisioned outside the repo.

No measured production memory/startup guarantee is claimed. Account auth, terminal, artifacts,
interactive approvals and worktree sessions remain unavailable in this release.
