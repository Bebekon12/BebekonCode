# Future remote client

There is no mobile app, TCP listener, HTTP server, relay or pairing implementation in V1.
The desktop's ClientTransport expresses public queries, commands and event subscriptions,
and Core contains no Tauri dependencies. This is the future integration seam.

A second client will require authenticated device pairing, encrypted transport, short-lived
device credentials, revocation, per-command authorization and bounded event replay by sequence.
Phone clients control sessions on the PC; provider access/refresh tokens must never leave it.
Remote commands must go through the same session and permission engine, including account
binding, approvals, cancellation and audit. Exposure of a socket must be an explicit feature,
not a side effect of extracting a standalone daemon.
