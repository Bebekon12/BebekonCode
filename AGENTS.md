# Repository instructions

Rust core is transport-independent. Tauri is a client adapter, never the owner of provider logic.
Preserve local-first behavior and explicit account binding. Never automatically rotate accounts or
retry with another account to evade limits. Never scrape provider websites, extract cookies,
reverse engineer authentication, impersonate providers, or use undocumented consumer endpoints.
Never copy credentials from official clients. Never log secrets or put them in SQLite, frontend,
configuration, Git, or crash metadata. Keep provider sandboxes and approvals enabled by default.
Exception approved by the owner: the user may explicitly switch an individual chat to full access
where the provider documents it (Codex `dangerFullAccess` with `approvalPolicy=never`). It is never
a default, is confirmed with a visible warning, never applies to read-only review/auto workers or
to providers without such a documented mode, and credentials stay off-limits.
Local computer-use tools (screen, mouse, keyboard) require per-action user confirmation and a
visible control indicator in every access mode, including full access.
No remote listener, telemetry, automatic merges, destructive Git actions, or other permission
bypasses.
Only official documented provider integrations. Unsupported features must be visible as unavailable.

Before changing provider behavior, check current official documentation and update the compliance
document. Validate with cargo fmt, cargo check, cargo test, cargo clippy -- -D warnings,
npm run typecheck, npm test, npm run build. Keep Cargo.lock and package-lock.json committed.
Do not publish an untested production claim. Releases must describe limitations and changes.
Published migrations and version tags are immutable. Add a new migration or release instead of
editing history. Pin GitHub Actions to resolved commit SHAs, not annotated tag-object SHAs.
