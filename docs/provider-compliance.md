# Provider compliance

Official documentation last checked: **2026-10-09**. This records technical integration research
and the resulting design. It is not a claim of provider approval or a legal audit.

Repository-wide prohibitions (also in [AGENTS.md](../AGENTS.md)):

- No web scraping or browser cookie extraction.
- No undocumented consumer APIs or provider impersonation.
- No reverse-engineered authentication or copied official-client credentials.
- No automatic quota rotation, hidden account fallback or rate-limit circumvention.
- No credential sharing or export.
- No secret logging and no plaintext token persistence in SQLite/config/frontend.
- No disabling provider safeguards by default.

## Windows sandbox and approvals (0.8.1, checked 2026-10-09)

Official references: [agent approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security),
[Windows sandbox](https://learn.chatgpt.com/docs/windows/windows-sandbox),
[app-server setup and approval protocol](https://learn.chatgpt.com/docs/app-server), and the
[official readiness response schema](https://github.com/openai/codex/blob/main/codex-rs/app-server-protocol/schema/typescript/v2/WindowsSandboxReadinessResponse.ts).
The installed official CLI 0.160.1 stable generated schema also includes `windowsSandbox/readiness`
with a null payload and `ready`, `notConfigured`, `updateRequired` responses.

- `workspace-write` plus `on-request` already is Codex's documented Auto preset. Both `standard`
  and `workspace_auto` retain that policy for Codex; UI states that confirming every individual
  Codex edit is unavailable. Retired `untrusted`/`unlessTrusted` settings are not introduced.
- On Windows, readiness is checked before every sandboxed Codex turn (including read-only workers). A
  missing, outdated or unknown sandbox stops the turn before inference with recovery instructions,
  instead of relying on a series of shell escalation prompts. Readiness is also shown per account.
- Only clicking the account's setup button invokes `windowsSandbox/setupStart` in `elevated`
  mode. The client waits for the matching completion notification and rechecks readiness. Setup
  is blocked while that account has active workers; new turns cannot race setup. Cancellation of
  a task waiting for setup is respected. Failure never selects an unelevated or unrestricted
  fallback. Timeout ends the idle app-server, leaving readiness to be checked on reconnection.
- Windows/UAC owns the administrator consent. The official legacy setup may create restricted
  users and isolation rules; agent commands do not run as administrator. Setup is never triggered
  by startup, status refresh, a model request or an automated test against the real Windows host.
  MXC settings from CLI 0.162+ are not forced onto the tested CLI 0.160.1.
- `availableDecisions`, when provided, restricts both UI buttons and backend decisions. Absent
  values preserve compatibility with old saved events. Managed network requests show the host
  and protocol instead of implying they are ordinary commands. `acceptForSession` is forwarded
  to Codex for the requesting agent only; it does not become a blanket grant for the team.
- Read-only runs decline escalation and file-change approval requests with a visible explanation;
  the reviewer cannot gain write access by accepting a prompt during a read-only review round.
- No command-text heuristic, automatic acceptance of server requests, authentication workaround,
  global permission grant or modification of another app's profile is used. The explicit per-chat
  full-access exception in 0.8.2 is described below.
- Claude's file hook retains the original Windows root spelling when reducing absolute paths
  to relative paths (including short-path aliases). The suffix still goes through the canonical
  root, credential-path denial and existing reparse-point/traversal checks. Reference rechecked:
  [official permissions and hooks](https://code.claude.com/docs/en/permissions). No CLI modes or
  allowed tools were expanded.

Validation includes simulated RPC setup completion/failure/busy cases and unsupported or wrong-agent
approval responses, plus the real CLI's readiness query in a fresh isolated profile without
inference. Real administrator-approved setup and authenticated model command execution require
the user's own interactive Windows session and remain unverified by the automated release checks.

## Full access and local computer use (0.8.2, owner decision 2026-10-09)

The repository owner approved changing the default-only safeguard rule in AGENTS.md. References
were checked by the team reviewer on 2026-10-09: [Codex permissions](https://developers.openai.com/codex/permissions/),
[app-server reference](https://learn.chatgpt.com/docs/app-server), [local MCP over stdio](https://learn.chatgpt.com/docs/extend/mcp)
and [ChatGPT-plan preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations/).

- `full_access` is a per-chat profile chosen manually after a red confirmation. It is never a
  default and is validated in the Rust core: only Codex (and the local demo) accept it.
- Codex receives `sandbox=danger-full-access` on `thread/start`/`thread/resume`,
  `sandboxPolicy={"type":"dangerFullAccess"}` and `approvalPolicy=never` on every `turn/start`.
  Windows sandbox readiness is not required in this mode. Switching back re-applies the restricted
  policy on the next resume and turn.
- Team review rounds and auto-mode workers are forced to `read_only` regardless of the member's
  profile, so a reviewer never inherits full access. Credentials and system settings remain
  prohibited by repository policy; unrestricted provider commands do not provide a filesystem
  isolation guarantee for these paths.
- Claude keeps its restricted mode, path hooks and deny rules; `full_access` is rejected for Claude
  and shown as unavailable in the UI. Its adapter still blocks MCP tools.
- Built-in Codex computer use and hosted tools are unavailable on the ChatGPT-plan route. A local
  computer-use MCP server (stdio) is planned: screenshots, pointer and keyboard actions, each
  confirmed by the user, with a visible indicator and stop control in every access mode. Not
  implemented yet; see `docs/computer-use.md`.

## Windows account skill paths (0.8.6, checked 2026-10-09)

Rechecked the official [Claude permissions reference](https://code.claude.com/docs/en/permissions).
Selected account skill roots retain the configured Windows spelling, including case differences
and short 8.3 aliases. Relative skill paths are derived from the canonical account root and still
pass the existing traversal, credential-name and reparse-point checks. This fixes the clean
Windows CI failure in 0.8.5 without adding tools, approval bypasses or authentication modes.
The regression checks both configured and canonical spellings. The v0.8.5 tag is preserved.

## Unity skills and Claude tool access (owner request 2026-10-09)

The owner asked to lift Claude restrictions where Anthropic's rules allow it. The team reviewer
checked [Claude sandboxing](https://code.claude.com/docs/en/sandboxing) and the CLI reference on
2026-10-09: skills, local plugins and MCP are documented features; on native Windows shell
commands run **without** an OS sandbox and MCP servers run outside it. Implemented in 0.8.5;
the real CLI 2.1.294 accepts the restricted startup in an isolated signed-out profile:

- **Skill references.** Read selected skill instructions through the guarded `Read` tool. Files come from an app-managed folder in the account's
  `CLAUDE_CONFIG_DIR`, not from the user's global profile. File tools may only read skill files;
  the credential and `.claude` path denials for project files stay.
- **Shell (PowerShell/Bash).** Offered in write profiles only. Every command goes through the
  existing PermissionRequest hook as a visible approval card with the exact command and folder,
  also in `workspace_auto` (no OS sandbox on Windows). Read-only participants never get shell.
- **MCP.** Only servers the user added in BebekonCode, passed with `--strict-mcp-config` and an
  explicit `--mcp-config`; tools named `mcp__<server>__*` are allowed per call through the same
  hook. Screen-capture tools (Unity MCP view capture may fall back to the whole desktop) always
  need a separate approval.
- **Full access for Claude remains unavailable.** The repository exception names Codex;
  `--restricted` also explicitly refuses `bypassPermissions`. The previous plan incorrectly
  treated documentation of a bypass flag as authorization to remove the guarded adapter.
  Write profiles retain restricted mode, file-path checks and individual shell/MCP approvals.
- **Unity.** The official Unity plugin 0.1.8-beta (Unity Technologies) ships 32 skills and documents
  manual skill installation. Many skills drive the editor through Unity CLI and
  `com.unity.pipeline` (0.6.0-exp.1+ required by the current CLI). `unity mcp configure codex`
  changes sandbox network settings and is never run automatically.

Implementation references re-fetched on 2026-10-09: [CLI reference](https://code.claude.com/docs/en/cli-reference),
[hooks](https://code.claude.com/docs/en/hooks), [environment variables](https://code.claude.com/docs/en/env-vars),
[Codex local MCP](https://learn.chatgpt.com/docs/extend/mcp), and Unity's
[official manual installation](https://github.com/Unity-Technologies/unity-agent-plugin#manual-install).
Restricted mode permits individually named shell tools via `--tools`. `--disable-slash-commands`
is retained on every run and the native `Skill` tool remains denied. Selected account skill names
and paths are passed with the documented `--append-system-prompt`; their files and resources can
be read on demand, with explicit `--add-dir` for those directories. This is app-managed reference
loading, not native skill execution. Account-managed references with executable hooks or forked
contexts are unavailable. Reads are checked against the chat's
selection; profile files remain unwritable through file tools. Shell and MCP are **not OS-sandboxed**
on native Windows: exact inputs require a fresh approval, never a session grant. The subprocess
environment scrub is enabled in addition to the existing environment allowlist.
Account profile directories (including other provider profiles) are blocked through file tools
even when the user chooses a parent directory as the project; only selected skill resources can
be read. Native slash/Skill activation is disabled, so a project skill cannot shadow the selected
account reference or register executable skill hooks. Frontmatter with hooks, contexts, escaped
keys or YAML aliases, and embedded skill plugins, is unavailable.

Re-fetched the official [headless bare-mode reference](https://code.claude.com/docs/en/headless#start-faster-with-bare-mode)
and CLI safe-mode reference before finalizing the implementation. `--bare` skips subscription
login and requires an API key; `--safe-mode` disables customization including MCP and skills.
Neither is enabled on this subscription route. No credentials are copied or converted to work
around these constraints. Native Skill execution remains visibly unavailable; reference loading
uses normal documented file tools and does not execute skill metadata.

New local stdio MCP definitions are stored per Claude account, without environment variables,
tokens or URLs. Saving a definition does not launch a server. Read-only workers receive no MCP.
Codex's existing configured MCP inventory is preserved; adding app-managed servers for Codex stays
unavailable until a bridge can enforce individual computer-use consent in full-access chats.
External local executables are user-selected and not an OS filesystem security boundary. No
credential isolation guarantee is claimed for shell or MCP processes on native Windows.

Local verification on 2026-10-09: 32 Unity skills from commit
`cf6b2da24e424b0a60d560a57f39f676cb6f79f3` installed separately into the existing OpenAI and Claude
profiles using Skill Installer. The selected Unity 6000.5.7f1 project resolved Pipeline
`0.8.0-exp.1` and official built-in Tilemap `1.0.0`. Both official Unity stdio MCP `editor_status`
and CLI `editor_status` succeeded. CLI `eval` created a separate rectangular test palette and
verified its `GridPalette` sub-asset. No screen capture or OS input was used. This proves the
Unity CLI/package connection on that project, not authenticated end-to-end tool invocation through
each provider adapter. That remains unverified; the release does not claim otherwise.

## OpenAI / Codex (implemented in 0.3.0)

Route: **Sign in with ChatGPT — ChatGPT plan usage for open-source and locally hosted apps**,
driving the official `codex app-server` over stdio exactly as that documentation describes.

- [Overview](https://developers.openai.com/siwc/token-sharing-open-source): the flow is for
  open-source and locally hosted apps; paid or remotely hosted apps must use OpenAI's interest
  form instead. BebekonCode is free, runs locally and is published under the repository's MIT licence.
- [Registration and sign-in](https://developers.openai.com/siwc/token-sharing-open-source/sign-in):
  dynamic registration with `client_id=dynamic_agent_client`, `agent_name_hint=BebekonCode`, a
  stable `ext_agent_host_id` (`urn:uuid:` per installation), PKCE S256, fresh `state`/`nonce`,
  `resource=https://api.openai.com/v1`, loopback callback on `127.0.0.1` (never `localhost`),
  ID-token validation against OpenAI JWKS (issuer, audience = issued client id, expiry, nonce),
  and the `chatgpt.tokens.use.direct` scope check before any inference.
- [Accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions):
  every account is a separate registration with its own issued client id and credentials, even
  with the same email. Refresh uses the issued client id and is serialized per account. Sign-out
  revokes the refresh token and reports when revocation could not be confirmed.
- [Codex app-server](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server):
  app-server is started with the documented `openai_chatgpt_plan` provider settings and the access
  token in `ACCESS_TOKEN`; `clientInfo.name` equals the `agent_name_hint`. Token renewal restarts
  the process and resumes threads with `thread/resume`. `model/list` is a catalog, not entitlement.
- [Errors and recovery](https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery)
  and [UI/UX guidelines](https://developers.openai.com/siwc/ui-ux-guidelines): usage limits stop
  work and link to [ChatGPT usage settings](https://chatgpt.com/settings/usage); the sign-in button
  reads "Continue with ChatGPT"; a one-time confirmation explains that the plan is used; the
  composer shows "Используется план ChatGPT" with a "Manage usage" link.
- [Preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations):
  image generation, file search, hosted MCP/connectors and similar hosted tools are unavailable
  on this route, so `generate_image` remains unavailable for OpenAI ChatGPT-plan accounts.

Usage and limits: this route documents no in-app usage API. The app shows limit snapshots only
if Codex itself reports them (`account/rateLimits/updated`) and otherwise links to ChatGPT usage
settings. No percentages are estimated.

Not used on purpose: Codex's own built-in ChatGPT login (`account/login/start`), the
`chatgptAuthTokens` mode marked "for OpenAI internal use only", and importing or reading any
`auth.json`. Sandbox is `workspace-write` or `read-only` with `approvalPolicy=on-request`, and
every approval request is shown to the user, except in the explicit per-chat full access mode
described in "Full access and local computer use" below.

Protocol facts (methods, fields, error codes) were taken from the schema generated by the installed
CLI (`codex app-server generate-json-schema`, codex-cli 0.160.1). The app-server protocol is marked
experimental; other CLI versions are flagged in the provider card.

## Chat controls and orchestration (0.4.0, checked 2026-10-06)

Primary references: [Codex App Server](https://learn.chatgpt.com/docs/app-server) and
[configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).
Protocol shapes are also checked against the official installed CLI 0.160.1 generated JSON schema.

- `model/list` reports `supportedReasoningEfforts` and `defaultReasoningEffort`; no invented levels.
- `turn/start` sets `model`, `effort`, `approvalPolicy=on-request` and the read-only/workspace-write
  `sandboxPolicy` on every turn. Network remains disabled by default. Returning to default effort
  sends that model's catalog default, rather than preserving the previous turn's override.
- Provider processes receive only their own account token. Agent shell subprocesses use
  `shell_environment_policy.inherit=core` and `ignore_default_excludes=false` so the inference
  token is not inherited by agent commands.
- `thread/start`/`thread/resume` accept thread-local `config`. MCP enablement and `skills.config`
  are applied there. `disabledPluginIds` in the installed CLI's `TurnStartParams` restricts installed
  plugins per thread. Account `config.toml` is never rewritten. Extension inventories are checked
  before applying a selection; a failed/incomplete inventory blocks the selection.
- Resume errors stop work, rather than silently creating an empty thread. Explicit handoff creates
  a fresh thread with a summary produced by the source account. The binding changes only after
  that succeeds. No automatic account changes or retry-on-another-account occur.
- Planning/summarising runs in a fresh read-only thread with extensions disabled; approval requests
  are denied. Task data is delimited as untrusted history, not inserted as developer instructions.
- Automatic compaction uses Codex's model defaults (`model_auto_compact_token_limit` unset).
  `contextCompaction` notifications are preserved as visible tool activity. No homemade token estimate.
- Teams use up to three configured participants, two concurrent child contexts and one review round.
  Auto plans use `outputSchema` and are validated before spawning at most four independent tasks.
  Workers are read-only; the coordinator applies changes through its normal approval policy.
  Outputs are bounded and errors stop synthesis; no fabricated worker success is substituted.
- ChatGPT-plan hosted image generation remains unavailable. No claim that model selection or a role
  gives a provider an unsupported tool.

## Claude adapter design (0.6.0, checked 2026-10-08)

The [legal guidance](https://code.claude.com/docs/en/legal-and-compliance) permits running the
unchanged binary with the end user's own authentication under the applicable Anthropic terms.
It does not authorize our own claude.ai OAuth flow, collecting tokens or reselling usage.
The [subscription notice](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan)
updated October 7 permits continued subscription-limit use; credits and entitlement remain
Anthropic's decision. No quota API or token extraction is used.

Route: installed, unmodified native Claude Code, `CLAUDE_CONFIG_DIR` per account, official
`auth login/status/logout`, and documented `-p` stream JSON. Credentials remain CLI-owned.
Require 2.1.293+ (verified locally with 2.1.294). Older installations stay unavailable.

- [Restricted mode](https://code.claude.com/docs/en/cli-reference): confine file tools to the
  project, ignore user/project settings, refuse permission bypass. Only Read/Glob/Grep/Edit/Write
  are offered. Shell, web, MCP, plugins and nested agents are unavailable on this Windows route.
- [Windows sandbox limits](https://code.claude.com/docs/en/sandboxing): native shell commands are
  unsandboxed; therefore none are exposed. Restricted file access is not an OS process sandbox.
- [Hooks](https://code.claude.com/docs/en/hooks): exec-form PreToolUse checks paths and forces
  write prompts; PermissionRequest forwards explicit user decisions over local-only named pipes.
  Missing replies deny. Read-only participants cannot write. No global permission rules change.
- [Headless](https://code.claude.com/docs/en/headless): normalize text deltas, result errors,
  session IDs, structured output and compaction. Ignore thinking and unknown messages.
- [Models](https://code.claude.com/docs/en/model-config): documented aliases and effort levels,
  explicitly labelled as choices, not an account-entitlement catalog. CLI rejects unavailable models.

Live inference requires the user's own login; fixture tests do not prove live subscription access.

## Anthropic integration references

- [Legal and compliance](https://code.claude.com/docs/en/legal-and-compliance): third-party apps may
  not offer claude.ai login of their own or route requests through Free/Pro/Max credentials on behalf
  of users, and may not collect, store or intermediate claude.ai credentials. An end user may sign in
  to the **unmodified** Claude Code binary with their own subscription, including when another
  product runs Claude Code. The binary must not be modified and its authentication methods must not
  be removed. Claude Code names and logos may not be used as BebekonCode's own branding.
- [Authentication](https://code.claude.com/docs/en/authentication): one `CLAUDE_CONFIG_DIR` per
  account keeps logins separate; sign-in happens through Claude Code's own flow
  (`claude auth login` / `/login`). BebekonCode must never read `.credentials.json` and must not use
  `claude setup-token` tokens.
- [Headless](https://code.claude.com/docs/en/headless) and [CLI reference](https://code.claude.com/docs/en/cli-reference):
  `claude -p` with `stream-json`, `system/init` (plugins, MCP servers), `--permission-prompt-tool`
  for approvals, `claude mcp` and `claude plugin` for per-profile extensions.
- [Status line](https://code.claude.com/docs/en/statusline): 5-hour and 7-day usage percentages are
  officially documented only for the interactive status line. The headless `rate_limit_event` is
  not documented; if shown, it must be labelled as unverified CLI output.

## Other references

- [Tauri updater](https://v2.tauri.app/plugin/updater/) requires update artifact signature verification.
- [Tauri GitHub distribution](https://v2.tauri.app/distribute/pipelines/github/) describes installer
  builds and release publishing with GitHub Actions.
- [GitHub Releases REST API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release)
  documents stable release metadata used by the manual checker.

Recheck these sources before changing provider behavior or shipping a new provider integration.

## Workspace access, attachments and visible progress (checked 2026-10-08)

### Settings, quota availability and Codex skill setup (checked 2026-10-09)

- Re-fetched official [SIWC accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions),
  [SIWC preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations),
  [Codex app-server](https://learn.chatgpt.com/docs/app-server) and
  [Build skills](https://learn.chatgpt.com/docs/build-skills). The SIWC account route documents
  a link to ChatGPT Settings → Usage, not a percentage polling API. The app-server quota method
  is documented for ChatGPT authentication, not our env-key SIWC provider. Refresh no longer
  calls that incompatible quota method. The UI explains this and offers the official usage page instead of promising a retry
  will provide percentages. No private usage endpoint or official-client credential is accessed.
- `plugin/install`, `plugin/uninstall`, `plugin/list` and `plugin/read` remain explicitly
  under development with "Don't call this method from production clients yet". No production
  plugin installation API has been enabled. Codex skill setup prepares a user-reviewed draft
  in a separate, explicitly selected account's standard-access chat invoking the documented
  `$skill-installer` workflow for the official `openai/skills` catalog. Nothing installs until
  the user sends the draft. Normal sandbox approvals apply; completion must be confirmed from
  the installer's result and refreshed inventory. This is skill setup, not marketplace plugin installation.
- Attachment preview returns only a raster image recorded in the requested chat's attachment
  events, inside that chat's `.bebekon-attachments` directory. Size limits, path boundaries and
  raster signatures are rechecked. It does not expose arbitrary local file URLs or expand Tauri asset scope.

### Follow-up verification (2026-10-08)

- Codex quotas are requested through documented `account/rateLimits/read` and updated from
  `account/rateLimits/updated`; failures mean unavailable, never a fabricated zero.
- Claude's [TypeScript reference](https://platform.claude.com/docs/en/agent-sdk/typescript#sdkratelimitevent)
  documents `rate_limit_event` (status, optional utilization/reset). These CLI events can be
  displayed without accessing credentials. They are not a documented polling API for both quota
  windows. The [status-line schema](https://code.claude.com/docs/en/statusline) includes 5h/7d
  percentages and effective effort, but availability in a restricted print session is unconfirmed.
- The same TypeScript reference documents `/usage` as a local command in SDK sessions. A native
  CLI 2.1.294 isolated-profile probe confirmed `local_command=usage`, `num_turns=0`,
  `duration_api_ms=0`, empty `modelUsage`, no inference. Account refresh invokes this command
  with no tools, no project settings/hooks/MCP/plugins and no session persistence. Only that
  explicitly identified zero-inference result is parsed. Recognized all-model session/week
  percentages become quota windows; other formats remain raw CLI status without guessed values.
  Reset text is displayed but is not guessed into a timestamp. Signed-in subscription data has
  not yet been verified against a real account on this installation.
- [CodexBar's own source documentation](https://github.com/steipete/CodexBar/blob/main/docs/claude.md)
  describes credential-file/Keychain access, private OAuth/web endpoints, and interactive CLI
  parsing. These do not establish provider authorization and are not adopted here.
- Public discovery uses the official GitHub marketplace manifests in `openai/plugins` and
  `anthropics/claude-plugins-official`. Browsing metadata does not install or execute plugins.
  Codex `plugin/list` and install methods are explicitly marked under development in the
  [app-server reference](https://learn.chatgpt.com/docs/app-server), so production code does
  not invoke them. Unsupported installation/execution stays visible.
- Model effort options follow the current [Claude model table](https://code.claude.com/docs/en/model-config#adjust-effort-level).
  Selected effort is a request, not proof of the applied value: managed settings can silently
  cap it in stream-JSON. The UI distinguishes requested/default effort from a reported value.
- Image attachments are native image inputs. Office text is extracted locally. Other files,
  including video/PDF/legacy Office, may be attached as local files; this does not imply native
  video understanding, OCR, audio transcription, or an installed converter. The UI states this.

Rechecked [Codex app-server](https://learn.chatgpt.com/docs/app-server),
[Claude CLI](https://code.claude.com/docs/en/cli-reference),
[Claude permissions](https://code.claude.com/docs/en/permissions),
[Claude model configuration](https://code.claude.com/docs/en/model-config) and
[headless operation](https://code.claude.com/docs/en/headless).

- `workspace_auto` is an explicit per-agent choice. Claude uses documented `acceptEdits`,
  retaining restricted tools, path-checking hooks and deny rules. Only validated project file
  writes are accepted automatically. Codex retains `workspace-write` and `on-request`;
  provider approval requests are always forwarded. (Superseded for Codex by the explicit
  full access mode in 0.8.2; Claude still has no unrestricted or bypass mode.)
- Team agents retain their selected access. Writers execute sequentially; read-only agents may
  run two at a time. The review round is read-only to avoid duplicate edits.
- Display actual Claude model IDs from `system/init.model` and `result.modelUsage`. Aliases are
  labelled as aliases; explicitly versioned choices are documented IDs, not entitlement claims.
- Visible progress means public status messages, tool activity, Codex reasoning summaries and
  actual worker messages/results. Never request private chain-of-thought or persist Claude
  thinking/signature blocks. No fabricated peer conversations.
- Attachments are explicit user input, bounded and validated by Rust core. Local file copies
  remain inside the selected workspace; images use documented Codex `localImage` input and
  Claude stream-JSON image content. Unsupported formats are shown as unavailable.
- Claude percentages are available only when the official zero-inference `/usage` response
  actually includes supported quota windows. No cookies, unofficial quota endpoints,
  guessed percentages or automatic account fallback are used.

This section supersedes earlier statements that all team workers are necessarily read-only.

## Team briefing and controls (0.7.0, checked 2026-10-08)

Rechecked the official [Codex app-server reference](https://learn.chatgpt.com/docs/app-server),
[Claude CLI reference](https://code.claude.com/docs/en/cli-reference), and
[Claude permissions](https://code.claude.com/docs/en/permissions). No new provider endpoints or
authentication modes are introduced. Model sliders use only the existing provider catalog.
The two enforced access profiles remain read-only and standard; parallel workers stay read-only.
Every team worker, review round and synthesis receives an explicit application briefing with
the roster, role, shared goal, actual round limits and the fact that there is no direct peer-call tool.
Questions travel in worker results through the existing bounded review round; the app does not claim
unrestricted autonomous delegation. Provider safeguards and account binding are unchanged.
Quota UI uses existing official snapshots only, with checked-at times and explicit unknown/error states.
Document review reads local allowlisted Office XML and text; app-owned comments are stored locally
in a new migration, never inserted into provider configuration or Office files.
