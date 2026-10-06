# Changelog

Every release links to a version tag and includes the same reviewed notes in GitHub Releases.

## 0.2.0 — 2026-10-06

### Added

- Real user-operated project file browser and UTF-8 text editor in the Windows desktop app.
- Create files/folders, save with Ctrl S, rename/move entries and confirmed permanent deletion
  of files/empty folders. Changes are written to the local disk through Rust IPC commands.
- Save conflict checks, same-directory staged replacement, unsaved-change decisions and bounds
  for text/file listings. Block traversal, Windows device names, junctions and `.git` internals.
- Open the selected project in Explorer or a visible external PowerShell terminal.
- Optional absolute `--data-dir` launch argument for separate local profiles and native QA.

### Changed

- Product name is now BebekonCode. A snowman with glasses, a blue scarf and a top hat appears in
  the desktop UI, Windows executable and NSIS/MSI installer icons. Existing app data is retained.

### Known limitations

- OpenAI/Claude adapters remain unavailable; the local demo is a simulation without file tools.
- The built-in editor supports UTF-8 text up to 2 MiB. Nonempty-folder deletion, other encodings
  and binary editing use external Windows tools. File removal is permanent, not the Recycle Bin.
- PowerShell opens externally; embedded terminal, isolated worktrees and agent tool approvals
  remain future work. Manual file operations are not an OS sandbox against hostile processes.
- Releases are not code-signed. GitHub update checks work; installation is still manual.

## 0.1.1 — 2026-10-06

### Fixed

- Pin the Tauri release action to the commit behind its annotated v1 tag, so the Windows release
  workflow uses a full commit SHA.

Includes the full local workspace development slice from 0.1.0 described below. Real OpenAI and
Claude adapters, worktrees and automatic installation remain unavailable. No runtime behavior
changes in this patch release.

## 0.1.0 — 2026-10-06

Initial development slice. This release is not the complete production V1.

### Added

- Rust/Tokio core separated from the React/Tauri 2 Windows desktop client.
- Local project folders, immutable account-bound sessions and SQLite/WAL migrations.
- Parallel simulated agent turns, streaming timeline, cancellation and persisted event replay.
- Graphite dark UI, context panel, provider/account/model controls and command palette.
- Official provider executable detection; provider adapters remain visibly unavailable.
- Read-only Git status and unified staged/unstaged diff with bounded process execution.
- Permission policy, canonical path containment, native credential store interface and redaction.
- Manual/opt-in startup GitHub release checks, SemVer comparison and release notes.
- Window state persistence, single-instance desktop, Windows NSIS/MSI packaging and CI workflows.

### Known limitations

- Only MockProvider runs. It never reads files, executes tools or modifies projects.
- OpenAI/Claude sign-in, inference, token refresh and multi-account login are not integrated yet.
- Worktrees, tool approvals, full terminal, capability delegation and artifacts are future milestones.
- Remote/mobile and automatic installation are unavailable. Updates are downloaded manually.
- Native signing certificates and Tauri automatic-update signing keys are not configured.
