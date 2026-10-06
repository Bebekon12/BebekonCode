# Provider compliance

Official documentation last checked: **2026-10-06**. This records technical integration research,
not a claim that the entire future product has received provider approval or a legal audit.
Only the local MockProvider runs in this release.

Repository-wide prohibitions:

- No web scraping or browser cookie extraction.
- No undocumented consumer APIs or provider impersonation.
- No reverse-engineered authentication or copied official-client credentials.
- No automatic quota rotation, hidden account fallback or rate-limit circumvention.
- No credential sharing or export.
- No secret logging and no plaintext token persistence in SQLite/config/frontend.
- No disabling provider safeguards by default.

These rules also appear in [AGENTS.md](../AGENTS.md).

Official references reviewed:

- [OpenAI Codex app-server and ChatGPT plan usage](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
  documents stdio initialization, app attribution, token replacement and thread resume.
- [Codex App Server](https://learn.chatgpt.com/docs/app-server) describes the official host protocol.
- [Claude Code programmatic use](https://code.claude.com/docs/en/headless) documents CLI JSON
  streaming, interruption and permission behavior; never infer that headless mode bypasses approvals.
- [Claude Code settings](https://code.claude.com/docs/en/settings) documents per-directory config
  through CLAUDE_CONFIG_DIR. Verify installed-version authentication behavior before enabling accounts.
- [Tauri updater](https://v2.tauri.app/plugin/updater/) requires update artifact signature verification.
- [Tauri GitHub distribution](https://v2.tauri.app/distribute/pipelines/github/) describes installer
  builds and release publishing with GitHub Actions.
- [GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)
  documents stable release metadata used by the manual checker.

Recheck registration, login, token storage/refresh and model availability immediately before
shipping real adapters. Do not treat a catalog as entitlement or a draft design as authorization.
