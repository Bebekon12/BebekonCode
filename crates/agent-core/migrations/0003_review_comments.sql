CREATE TABLE review_comments (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id),
    path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    anchor TEXT NOT NULL,
    quote TEXT NOT NULL,
    body TEXT NOT NULL,
    resolved INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX review_comments_file ON review_comments(workspace_id, path, created_at);
