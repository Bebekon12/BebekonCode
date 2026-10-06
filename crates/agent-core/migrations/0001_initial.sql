CREATE TABLE workspaces (
    id TEXT PRIMARY KEY, name TEXT NOT NULL, root TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);
CREATE TABLE account_profiles (
    id TEXT PRIMARY KEY, provider TEXT NOT NULL, label TEXT NOT NULL,
    credential_ref TEXT, config_dir TEXT, auth_status TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    provider TEXT NOT NULL, account_profile_id TEXT NOT NULL REFERENCES account_profiles(id),
    model TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL,
    permission_profile TEXT NOT NULL, working_directory TEXT NOT NULL,
    provider_session_id TEXT, worktree_id TEXT,
    created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    run_id TEXT NOT NULL, timestamp INTEGER NOT NULL, payload TEXT NOT NULL
);
CREATE INDEX events_session_sequence ON events(session_id, sequence);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE capability_bindings (
    capability TEXT PRIMARY KEY, provider TEXT NOT NULL,
    account_profile_id TEXT NOT NULL REFERENCES account_profiles(id)
);
CREATE TABLE capability_audit (
    id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id),
    capability TEXT NOT NULL, provider TEXT NOT NULL, account_profile_id TEXT NOT NULL,
    status TEXT NOT NULL, created_at INTEGER NOT NULL
);
INSERT INTO account_profiles VALUES (
    'mock-local', 'mock', 'Local demo', NULL, NULL, 'not_required', 0
);
INSERT INTO settings VALUES ('check_updates_on_start', 'false');
