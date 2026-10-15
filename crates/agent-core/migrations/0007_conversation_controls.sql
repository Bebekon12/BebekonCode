ALTER TABLE sessions ADD COLUMN conversation_settings TEXT NOT NULL DEFAULT '{}';
ALTER TABLE queued_tasks ADD COLUMN attachments TEXT NOT NULL DEFAULT '[]';
ALTER TABLE queued_tasks ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
UPDATE queued_tasks SET position=rowid;
