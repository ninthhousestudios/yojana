CREATE TABLE IF NOT EXISTS tasks (
  id                  BLOB PRIMARY KEY,
  project_id          BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  sequence_number     INTEGER NOT NULL,
  title               TEXT NOT NULL,
  description         TEXT NOT NULL DEFAULT '',
  category            TEXT,
  status              TEXT NOT NULL DEFAULT 'needs-triage',
  slice_type          TEXT,
  acceptance_criteria TEXT NOT NULL DEFAULT '[]',
  decisions           TEXT NOT NULL DEFAULT '[]',
  implementation_plan TEXT,
  execution_record    TEXT,
  reproduction        TEXT,
  root_cause          TEXT,
  context_refs        TEXT NOT NULL DEFAULT '[]',
  files               TEXT NOT NULL DEFAULT '[]',
  tags                TEXT NOT NULL DEFAULT '[]',
  history             TEXT NOT NULL DEFAULT '[]',
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL,
  UNIQUE (project_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS tasks_project_status_idx ON tasks (project_id, status);
