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

The first release is explicitly early development even though it is visible on the stable
metadata channel. There is no automatic installation. Preview/draft releases are excluded by
the app's `/releases/latest` checker. A 404 means no public stable release is available (also
possible for inaccessible private repos); a rate-limit/network error is visible and is not
reported as “up to date”. Notes are displayed as text, never raw executable HTML.

Before enabling automatic updates, generate a persistent Tauri signing key **outside the repo**,
store the private key/password in protected GitHub Actions secrets, keep an offline recovery
copy, commit only the public key, enable the official plugin and signed updater manifests,
and test signature failures, target compatibility and upgrade recovery. Never commit keys or
accept unsigned automatic payloads. Windows Authenticode signing requires separate provisioning.
