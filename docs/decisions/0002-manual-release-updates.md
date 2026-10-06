# ADR 0002: GitHub release tracking and manual installation first

Accepted 2026-10-06.

Use GitHub Releases as the distribution and changelog source. The app compares stable SemVer
metadata and opens a trusted release URL. It does not execute an installer or silently replace
the running app. Startup checking is opt-in; no constant polling or telemetry.

Reason: no persistent private signing key has been provisioned. Shipping a custom automatic
updater would violate the security requirements. Add the official Tauri updater only after
secure signing-key management and recovery are established. Windows Authenticode signing is
separate from Tauri update signatures and remains unconfigured in this initial release.
