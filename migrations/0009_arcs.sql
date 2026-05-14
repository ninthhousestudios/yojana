CREATE TABLE IF NOT EXISTS arcs (
  id              BLOB PRIMARY KEY,
  project_id      BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  sequence_number INTEGER NOT NULL,
  title           TEXT NOT NULL,
  description     TEXT NOT NULL DEFAULT '',
  status          TEXT NOT NULL DEFAULT 'active',
  phases          TEXT NOT NULL DEFAULT '[]',
  tags            TEXT NOT NULL DEFAULT '[]',
  context_refs    TEXT NOT NULL DEFAULT '[]',
  history         TEXT NOT NULL DEFAULT '[]',
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  UNIQUE (project_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS arcs_project_status_idx ON arcs (project_id, status);

ALTER TABLE tasks ADD COLUMN arc_id BLOB REFERENCES arcs(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN arc_phase TEXT;
