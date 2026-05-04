CREATE TABLE IF NOT EXISTS task_conversations (
    id          BLOB PRIMARY KEY,
    task_id     BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    messages    TEXT NOT NULL DEFAULT '[]',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_conversations_task
    ON task_conversations(task_id);
