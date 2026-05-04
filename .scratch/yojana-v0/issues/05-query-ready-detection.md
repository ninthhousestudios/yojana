Status: needs-triage

# 05 — Query + ready detection

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Add `yojana_query` and `yojana_ready` MCP tools. Query supports filtering tasks by status, tags, project, and blocked/ready state. Ready detection identifies tasks where all `depends_on` targets are `done`.

- Graph engine: `ready_tasks(project_id?) → Vec<Task>` — tasks with all depends_on edges satisfied (targets done) and own status is ready-for-agent or ready-for-human
- Graph engine: `blocked_by(task_id) → Vec<Task>` — which incomplete dependencies block this task
- `yojana_query` tool: filter by project, status, tags, slice_type, category; include blocked/ready flag per result
- `yojana_ready` tool: shortcut for "what can start now?" across one or all projects
- Cross-project support: omit project filter to query across everything

## Acceptance criteria

- [ ] `yojana_query` filters by project (optional), status, tags, slice_type, category
- [ ] Each query result includes a `blocked` flag and `ready` flag computed from the dependency graph
- [ ] `yojana_ready` returns only tasks that are ready-for-agent or ready-for-human with all depends_on targets done
- [ ] Cross-project query works when project filter is omitted
- [ ] `yojana_ready` with no project returns ready tasks across all projects
- [ ] Graph engine ready-detection tested with various topologies: no deps (always ready), chain, diamond, partial completion

## Blocked by

- 03 — State machine
- 04 — Edges + cycle detection
