Status: needs-triage

# 02 — Task CRUD + sequence numbers

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Add the tasks table and `yojana_task` MCP tool. Tasks belong to a project and get a per-project sequence number (e.g. YJN-42). CRUD operations support all task fields including JSON columns (acceptance_criteria, decisions, context_refs, files, tags, history).

- Migration 0002: tasks table with all columns from the schema
- Store module: task create, get, update (no delete — tasks are closed, not deleted)
- Per-project sequence number generation (atomic increment, no gaps on failure not required)
- `yojana_task` MCP tool with create, get, update actions
- JSON column round-tripping for acceptance_criteria, decisions, context_refs, files, tags, history
- context_refs shape validation against the allowlist (smriti:hash, smriti:path, sutra:symbol, kosha:citation, yojana:task, chitta:memory, doc:path, git:commit, git:range)

## Acceptance criteria

- [ ] Tasks are created within a project; project_id is required
- [ ] Each task gets an auto-incrementing per-project sequence number
- [ ] `yojana_task action=get` accepts both UUID and project-slug + sequence number (e.g. "yojana/42")
- [ ] JSON columns round-trip correctly (store and retrieve structured data)
- [ ] context_refs validates type against the allowlist; rejects unknown types
- [ ] `yojana_task action=update` supports partial updates (only specified fields change)
- [ ] Store tests cover CRUD, sequence number uniqueness, JSON round-tripping, cascade delete when project is deleted
- [ ] Category field accepts null, "bug", "enhancement", "experiment"

## Blocked by

- 01 — HTTP skeleton + project CRUD
