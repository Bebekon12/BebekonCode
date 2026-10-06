# Security model

The desktop is local-first: no analytics, crash upload, cloud account or remote gateway.
External requests: the opt-in GitHub release check, and for OpenAI accounts the documented Sign in
with ChatGPT endpoints on `auth.openai.com` plus the requests the Codex CLI itself makes.
The only listener is the one-shot OAuth callback on `127.0.0.1` during an explicit sign-in: it is
bound before the browser opens, accepts only `/auth/callback`, validates `state` and closes after
one callback or ten minutes. The mock agent never reads files, executes commands or contacts a
provider.

Tauri capabilities grant only core UI events/window behavior and the native open-folder dialog.
There is no generic frontend shell, filesystem-plugin or HTTP API. Typed project-scoped file
commands run in Rust only for user actions; they are not available as agent tools. CSP restricts scripts to packaged assets,
forbids frames and objects, and restricts connect-src to IPC (and loopback HMR in development).
Prompts/provider text are rendered as escaped React text, never executable HTML. Browser preview
requires a development build and `?preview=1`; its in-memory state is clearly labeled.

SQLite contains workspace/session/account metadata and user-visible timeline content, not
provider credentials. Transcripts are private local data and are not encrypted at rest; OS user
profile permissions and disk encryption protect them. Do not paste credentials into prompts.
OpenAI tokens (access, refresh, ID) live in one record per account, encrypted with Windows DPAPI
(user scope) and written atomically inside that account's profile folder. No IPC command returns
them; they are only placed in the environment of that account's Codex process (`ACCESS_TOKEN`).
Windows Credential Manager is not used for them because its entries are limited to 2.5 KB.
Sign-out revokes the refresh token at OpenAI; removing an account deletes its folder.

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
commands are exposed.

Provider processes start from an absolute executable path with an allowlisted environment, so API
keys or config overrides for other providers and accounts never reach them. Each provider process
tree is placed in its own kill-on-close Job Object; stopping an account, closing the app or a crash
ends the tree, including helpers the CLI starts itself (Codex runs `git` for its plugin catalog).
Pipes are read on dedicated OS threads so an inherited handle cannot block shutdown. Processes that
BebekonCode did not start are never touched.

GitHub checks use HTTPS with no redirects, 12-second timeout, 256-KiB response limit and a six-hour
in-process cache for non-forced checks. Preview/draft/malformed versions are rejected. Release
links are constructed from the configured repository and a validated SemVer tag; returned URLs
are never executed. Updates are manual. A future auto-installer must use Tauri signature checking.
