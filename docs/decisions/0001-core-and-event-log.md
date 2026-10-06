# ADR 0001: Transport-independent core and durable normalized events

Accepted 2026-10-06.

Provider and session behavior lives in a separate Rust crate. Tauri commands are thin adapters.
In the first slice the core is embedded in the desktop process, avoiding a service and network
surface. Normalized events commit to SQLite before UI notification; sequence IDs allow replay
and deduplication. Account identity is immutable within a session.

Consequences: a UI reload preserves live core tasks; a desktop process exit stops them and
the next launch marks unfinished sessions interrupted. A standalone daemon and remote transport
remain possible without rewriting providers. The event log requires pagination and later
compaction/message snapshots for very large real-provider transcripts.
