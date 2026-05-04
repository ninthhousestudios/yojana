CREATE TABLE IF NOT EXISTS task_edges (
  id              BLOB PRIMARY KEY,
  source_task_id  BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  target_task_id  BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  edge_type       TEXT NOT NULL,
  note            TEXT,
  created_at      INTEGER NOT NULL,
  UNIQUE (source_task_id, target_task_id, edge_type)
);

CREATE INDEX IF NOT EXISTS edges_source_idx ON task_edges (source_task_id);
CREATE INDEX IF NOT EXISTS edges_target_idx ON task_edges (target_task_id);
