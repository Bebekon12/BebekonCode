CREATE TABLE queued_tasks (
 id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 prompt TEXT NOT NULL, configuration TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'queued',
 run_id TEXT, error TEXT, created_at INTEGER NOT NULL
);
CREATE INDEX queue_order ON queued_tasks(state, created_at);
CREATE TABLE checkpoints (
 id TEXT PRIMARY KEY, session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 root TEXT NOT NULL, label TEXT NOT NULL, created_at INTEGER NOT NULL,
 file_count INTEGER NOT NULL, limited INTEGER NOT NULL
);
