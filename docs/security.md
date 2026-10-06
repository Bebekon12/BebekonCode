# Security model

The desktop is local-first: no analytics, crash upload, listener, cloud account or remote gateway.
The only implemented external request is an explicit or opt-in GitHub release metadata check.
The mock agent never reads files, executes commands, contacts a provider or changes a project.

Tauri capabilities grant only core UI events/window behavior and the native open-folder dialog.
There is no generic frontend shell, filesystem-plugin or HTTP API. Typed project-scoped file
commands run in Rust only for user actions; they are not available as agent tools. CSP restricts scripts to packaged assets,
forbids frames and objects, and restricts connect-src to IPC (and loopback HMR in development).
Prompts/provider text are rendered as escaped React text, never executable HTML. Browser preview
requires a development build and `?preview=1`; its in-memory state is clearly labeled.

SQLite contains workspace/session/account metadata and user-visible timeline content, not
provider credentials. Transcripts are private local data and are not encrypted at rest; OS user
profile permissions and disk encryption protect them. Do not paste credentials into prompts.
The native credential interface uses Windows Credential Manager; no IPC command exposes it.
No real tokens are collected in this slice. Credential login/refresh remain future work.

Logs use static allowlisted codes; prompt text, paths, provider JSON and raw errors are not logged.
The central redaction writer masks sensitive formatted log records before output, including
authorization, token, API key, cookie, password and secret fields. Tests cover leakage.
Redaction is defense in depth; never intentionally log a secret. Secret runtime memory hardening
and crash dump handling need review before real authentication is shipped.

Paths are canonicalized before a workspace is saved. Existing-file containment resolves symlinks
and junctions and checks path components. This is a policy check, not an OS filesystem sandbox;
real providers must keep their own sandbox and approval controls. The user file manager validates
relative components and existing parents, rejects links/reparse points, device names, traversal,
alternate data streams and `.git` internals. Text reads and writes are capped at 2 MiB; folder
listings at 5000 entries. Writes compare expected content and stage/sync a same-directory file
before replacement ([MoveFileExW on Windows](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)). IPC file operations are serialized. Deletion is
nonrecursive and requires a displayed-path confirmation in the UI. These path/content checks
are not atomic against a malicious external process replacing directories between checks.
Inherited/custom ACLs and hard-link identity can change on replacement. This is a manual editor
boundary, not permission to reuse these operations as a sandbox for an untrusted agent.

Explicit user clicks can open Explorer and a visible external PowerShell console for the
selected project. PowerShell receives a working directory and a fixed `-NoExit` argument;
there is no interpolation of filenames into a shell command or frontend command execution API.

Read-only Git calls use a directly resolved executable, no shell, clean allowlisted environment,
null stdin/stderr, ten-second timeout and a two-MiB output limit. fsmonitor is disabled to avoid
repository-controlled helper execution; external diff and textconv are disabled. No Git mutation
commands are exposed. Full process-tree supervision is a prerequisite for real agent adapters.

GitHub checks use HTTPS with no redirects, 12-second timeout, 256-KiB response limit and a six-hour
in-process cache for non-forced checks. Preview/draft/malformed versions are rejected. Release
links are constructed from the configured repository and a validated SemVer tag; returned URLs
are never executed. Updates are manual. A future auto-installer must use Tauri signature checking.
