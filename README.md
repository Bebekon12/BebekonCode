# BebekonCode · AI developer workspace

A local Windows desktop workspace for coding agents. Rust core, Tauri 2, React and SQLite.
Source repository: [BebekonCode](https://github.com/Bebekon12/BebekonCode).

**0.2.0 is a development release, not the complete production V1.** The local demo works:
add a project, create independent sessions, send messages, watch streaming, stop a turn and reopen
saved history. Read your actual Git status/diff. OpenAI and Claude are detected but not connected;
the demo does not inspect or edit project files. The separate user-operated file manager reads
and edits real project files. No credentials are required to try it.

## Install the Windows app

Download `BebekonCode_0.2.0_x64-setup.exe` from [GitHub Releases](https://github.com/Bebekon12/BebekonCode/releases).
For a standalone launch, use `BebekonCode_0.2.0_x64.exe` (WebView2 Runtime must already be installed).
The installed app runs as `bebekoncode-desktop.exe` with a bundled UI in WebView2. End users do
not run a browser, npm, Node or a development server. WebView2 is the Windows rendering runtime.

Select a project and click **Project files** to browse local folders, create files/folders,
edit UTF-8 text, save with Ctrl S, rename/move entries, or permanently delete a file/empty folder
after confirmation. Explorer opens the project in Windows; PowerShell opens a visible native
terminal with the project as its working directory. The terminal is external, not embedded.
Files larger than 2 MiB, binary files, other encodings and nonempty-folder deletion use external
Windows tools. Junctions, links, reserved names, traversal and `.git` internals are blocked in
the built-in file manager. Saves check for external content changes and replace the file from
a same-directory temporary file; unsaved edits require a discard decision before navigation.

## Requirements

For development: Windows 11 x64, WebView2 Runtime, Rust stable (MSVC), Visual Studio 2022 Build Tools with Desktop
development with C++, Windows SDK, Node.js 22+ and npm. Git is optional for the app, required to
develop this repository. No Electron, Chromium service, Node backend or SQLite service is used.

## Run

```powershell
npm ci
npm run dev
```

The launcher finds the usual `%USERPROFILE%\.cargo\bin` when Cargo is absent from PATH.
For frontend-only visual development use `npm run dev:ui` and
`http://localhost:1420/?preview=1`. This explicitly labeled preview uses memory instead of SQLite
and cannot access files or providers. Production browser loading is disabled.

## Build and package

```powershell
npm run build
npm run package
```

Installers are emitted to `target/release/bundle/nsis` and `target/release/bundle/msi`.
Uninstall removes the application, not your project folders. App data is retained.

## Test

```powershell
cargo fmt --all --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
npm run typecheck
npm test
npm run build
npx playwright install chromium
npm run test:e2e
```

For an installed Edge instead of Playwright's browser: set `$env:PLAYWRIGHT_CHANNEL = 'msedge'`.
Browser tests cover the development UI adapter; Rust tests validate persistence and runtime.
After packaging, `npm run test:native` checks the actual executable's WebView2/IPC and file CRUD
using an isolated temporary profile. Only that test process enables a loopback debug connection.
These tests are not full installer automation or real provider end-to-end tests.

## Releases and updates

[GitHub Releases](https://github.com/Bebekon12/BebekonCode/releases) are the distribution source.
See [release procedure](docs/releases.md) and [CHANGELOG.md](CHANGELOG.md). Each version is
tracked by a `vX.Y.Z` Git tag, a reviewed changelog entry and Windows installers.
Settings → About & updates checks the latest stable release and displays its notes. Startup
checks are opt-in. Downloads and installation are manual in 0.1.0; automatic installation
will use the official signed Tauri updater after signing is configured.

## Architecture, provider setup and security

- [Architecture and next milestones](docs/architecture.md)
- [Security boundaries and limitations](docs/security.md)
- [Provider setup and planned official integrations](docs/providers.md)
- [Permission profiles](docs/permissions.md)
- [Provider compliance and official documentation](docs/provider-compliance.md)
- [Future mobile transport](docs/mobile-future.md)

App state lives under `%LOCALAPPDATA%\com.bebekon.agent-workspace\workspace.db`.
An explicit `--data-dir D:\absolute\folder` launch option selects a separate local data profile,
including for isolated native smoke tests. The stable default identifier preserves existing data.
SQLite stores metadata and private local transcripts; provider secrets belong only in native
secure storage. No telemetry, remote listener, automatic account rotation or permission bypass.

Product identity is configured in `product.json` and `src-tauri/tauri.conf.json`; keep the stable
identifier when renaming. UI settings currently provide the implemented controls and explicitly
label future integrations, rather than exposing inactive login or execution buttons.
