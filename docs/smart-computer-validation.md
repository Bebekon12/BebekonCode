# Smart Computer validation

## Stage 1 — protocol, policy engine, local transport (2026-10-10)

- `cargo fmt --all`: passed.
- `cargo test -p smart-computer`: 8 tests passed. Includes cancellation before capture,
  explicit account/session binding, read-only denial, separate permissions for sequence steps,
  stop on failed verification, strict request parsing, PNG encoding, crop/change bounds and
  bounded frame cache. Native desktop operations were not invoked by these tests.
- `cargo check --workspace`: passed, including the Tauri adapter and Codex configuration bridge.
- `cargo clippy -p smart-computer -- -D warnings`: passed.
- `npm run typecheck`: passed with the opt-in control panel and approval indicator.

These are development checkpoints. Native Windows execution, live subscription MCP images,
Unity compilation/Undo and final full-suite checks are pending; no production-ready claim.
