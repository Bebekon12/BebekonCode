# Validation record: 0.1.0

Date: 2026-10-06. Environment: Windows x64, Rust 1.99.0 stable/MSVC, Node 24.12.0.

Passed locally:

- cargo fmt, cargo check for the whole workspace, cargo clippy with warnings denied.
- 11 Rust tests: permission defaults and containment, secret redaction, SemVer/release metadata,
  delegation limits, real Git status/diff, parallel session cancellation, account validation,
  persistent history and interrupted-run recovery.
- Strict TypeScript checking, two event-replay unit tests, Vite production build.
- Three Playwright UI tests using installed Edge: project/session creation, streaming, stop,
  unavailable provider selection, command palette, settings/release display, independent
  timelines and compact logical viewport containment. No page errors in the main scenario.
- npm production dependency audit: no reported vulnerabilities.
- Tauri Windows release build, NSIS and MSI packaging (approximately 4.5 and 6.4 MiB).
- Native packaged executable startup: window created, SQLite created, graceful exit code 0.
- Prettier formatting, version consistency and staged Git whitespace checks.

Network resets interrupted initial downloads of test tools. UI tests use installed Edge as
documented; no additional browser runtime is used by the application. Rust tests use an owned
temporary-directory fixture and no external provider credentials.

Limits of this evidence: UI browser tests use the explicitly labeled development transport.
Native startup is a smoke check, not complete WebView2 IPC or installer automation. MSI/NSIS
clean-install/uninstall QA, memory/CPU benchmarks, real provider inference/auth, Windows Job
Object supervision and automatic-update signatures are not claimed as validated.
