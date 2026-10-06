# Repository instructions

Rust core is transport-independent. Tauri is a client adapter, never the owner of provider logic.
Preserve local-first behavior and explicit account binding. Never automatically rotate accounts or
retry with another account to evade limits. Never scrape provider websites, extract cookies,
reverse engineer authentication, impersonate providers, or use undocumented consumer endpoints.
Never copy credentials from official clients. Never log secrets or put them in SQLite, frontend,
configuration, Git, or crash metadata. Keep provider sandboxes and approvals enabled.
No remote listener, telemetry, automatic merges, destructive Git actions, or permission bypasses.
Only official documented provider integrations. Unsupported features must be visible as unavailable.

Before changing provider behavior, check current official documentation and update the compliance
document. Validate with cargo fmt, cargo check, cargo test, cargo clippy -- -D warnings,
npm run typecheck, npm test, npm run build. Keep Cargo.lock and package-lock.json committed.
Do not publish an untested production claim. Releases must describe limitations and changes.
