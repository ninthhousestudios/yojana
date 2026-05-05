ALTER TABLE projects ADD COLUMN parent_id BLOB REFERENCES projects(id);
