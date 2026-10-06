# Release procedure

Repository: https://github.com/Bebekon12/BebekonCode

1. Update version in root package.json and Cargo.toml; refresh lockfiles.
   Tauri reads its version from package.json. `npm run check:versions` checks the workspace and tag.
2. Add a dated, reviewed CHANGELOG.md section with features, fixes and known limitations.
3. Run the README validation commands and build/test Windows installers. Verify a clean install,
   launch, retained history on relaunch, shutdown and uninstall without touching project folders.
4. Commit and push main. Tag exactly `vX.Y.Z` and push that tag. The Windows release workflow first
   runs validation, then builds NSIS/MSI, and publishes the release with the reviewed changelog.
5. Inspect the workflow result and published installer assets before announcing availability.

Example after checks:

```powershell
git tag -a v0.1.0 -m "Initial local workspace development slice"
git push origin main
git push origin v0.1.0
```

Published tags are immutable in normal use. Fix a release with a new version; do not force-push
or replace an existing tag. GitHub Actions uses its scoped GITHUB_TOKEN, not a committed PAT.
Workflow dependencies are pinned by commit SHA. Updates should refresh pins after review.

## Signed in-app updates (from 0.3.2)

The official Tauri updater reads
`https://github.com/Bebekon12/BebekonCode/releases/latest/download/latest.json`.
NSIS `setup.exe` is the update payload (`updaterJsonPreferNsis: true`). The manifest contains
its signature and a versioned HTTPS asset URL. Updates use Windows `passive` mode: a small
progress window, no setup wizard. NSIS restarts the app after installation. The app downloads
only after confirmation and never installs if signature verification fails. Active sessions
block installation. Signed version metadata must match the manifest version; the core shuts down idle provider processes before launching the installer.
Checks have a 30-second timeout, downloads a 120-second timeout. Retry requires a fresh check.

The key was generated outside the repository in `%USERPROFILE%\.bebekoncode\signing`.
Only the public key belongs in `tauri.conf.json`. GitHub Actions secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` hold the encrypted signing
material. Keep a secure offline backup: losing the key breaks updates for existing installations.
Never print keys or passwords in logs. Tauri signatures are separate from Windows Authenticode.

Local signed build (PowerShell, no secrets in command arguments):

```powershell
$signingFolder = Join-Path $env:USERPROFILE '.bebekoncode\signing'
try {
  $env:TAURI_SIGNING_PRIVATE_KEY = Join-Path $signingFolder 'bebekoncode-updater.key'
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = (Get-Content -LiteralPath (Join-Path $signingFolder 'bebekoncode-updater.password') -Raw).Trim()
  npm run package
} finally {
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}
```

The release workflow uploads installers, `.sig` files and `latest.json` to a draft; it checks the
manifest version, NSIS payload and canonical asset URL before publishing the complete release.
Published tags remain immutable. Fixes use a new version. Preview/draft releases are excluded
from `/releases/latest`. A network/signature/permission error must remain visible.

Upgrading from 0.3.1 or older requires one manual installer run because those versions do not
contain the updater. Future upgrades are initiated inside Settings. Test a complete installed
upgrade separately before claiming installation QA; do not replace a user's running app for tests.

Sources: [Tauri updater](https://v2.tauri.app/plugin/updater/),
[pinned release action inputs](https://github.com/tauri-apps/tauri-action/blob/1deb371b0cd8bd54025b384f1cd735e725c4060f/action.yml).

Local package verification after building:

```powershell
node scripts/release-notes.mjs
node scripts/updater-manifest.mjs
npm run test:updater
```

This runs the official native updater against a test-only loopback fixture, verifies the actual
installer signature, rejects a modified installer and a different advertised version, and never
installs. `updater_audit` is an example executable, not part of the shipped app. Production uses
HTTPS only and the GitHub endpoint. Already published assets are preserved by workflow retries.
