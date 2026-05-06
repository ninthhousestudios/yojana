ALTER TABLE tasks ADD COLUMN completed_at INTEGER;

-- Backfill: derive completed_at from history for tasks already in 'done' status.
-- Each history entry is JSON {ts, kind, payload:{from,to}}. Take the most recent
-- transition into 'done' as the completion timestamp.
UPDATE tasks
SET completed_at = (
  SELECT MAX(json_extract(value, '$.ts'))
  FROM json_each(tasks.history)
  WHERE json_extract(value, '$.kind') = 'status_changed'
    AND json_extract(value, '$.payload.to') = 'done'
)
WHERE status = 'done';

CREATE INDEX IF NOT EXISTS tasks_completed_at_idx ON tasks (completed_at);
