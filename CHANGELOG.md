# Changelog

Every release links to a version tag and includes the same reviewed notes in GitHub Releases.

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
