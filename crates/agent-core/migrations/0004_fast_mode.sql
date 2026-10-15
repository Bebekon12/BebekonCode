-- Explicit per-agent preference; existing chats keep standard speed.
ALTER TABLE sessions ADD COLUMN fast_mode BOOLEAN NOT NULL DEFAULT 0;
