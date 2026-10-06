# Validation record

## 0.2.0 — 2026-10-06

Passed locally on Windows x64, Rust 1.99.0 MSVC, Node 24.12.0:

- Whole-workspace Cargo fmt/check/test/clippy with warnings denied and locked dependencies.
- 14 Rust tests including actual disk CRUD, external-save conflicts, reserved/traversal names,
  binary rejection and a real Windows junction escape attempt with outside-file preservation.
- Strict TypeScript checking, two replay unit tests, production build and three Edge UI tests.
- Actual release executable tested in WebView2 via `npm run test:native`: real Tauri IPC and
  disk create/read/save/rename/delete, Unicode/spaces in paths, CRLF preservation, external
  save conflict rejection, protected `.git`, traversal denial and dirty-navigation decisions.
- Native window-close request with unsaved edits was prevented; keeping edits retained the
  buffer. Global command shortcuts did not replace the editor and discard its text.
- Native test used an isolated temporary data profile. Default app data was not altered.
- NSIS `.exe` and MSI packaging succeeded (about 5.5 and 7.5 MiB); standalone executable is
  about 18.6 MiB. Logo output was inspected and transparent corner alpha confirmed as zero.
- Prettier, version consistency and Git diff whitespace checks passed.

Native CDP is enabled only by the smoke harness environment for its own process, not by the
application. A native editor screenshot is emitted to `test-results/native-files.png`.
Full clean-install/uninstall/upgrade QA, external-console automation, DPI/display coverage,
custom file ACL preservation, hostile-process race resistance and real provider workflows are
not established by these tests. Signing and automatic update installation remain unavailable.

## 0.1.0

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
