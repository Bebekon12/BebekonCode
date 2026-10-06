-- Preserve every existing conversation and provider thread.
ALTER TABLE sessions ADD COLUMN reasoning_effort TEXT;
ALTER TABLE sessions ADD COLUMN tool_policy TEXT NOT NULL DEFAULT '{}';
ALTER TABLE sessions ADD COLUMN parent_session_id TEXT REFERENCES sessions(id);
ALTER TABLE sessions ADD COLUMN chat_mode TEXT NOT NULL DEFAULT 'single';
ALTER TABLE sessions ADD COLUMN role TEXT NOT NULL DEFAULT '';
ALTER TABLE sessions ADD COLUMN context_summary TEXT NOT NULL DEFAULT '';
CREATE INDEX sessions_parent ON sessions(parent_session_id);
