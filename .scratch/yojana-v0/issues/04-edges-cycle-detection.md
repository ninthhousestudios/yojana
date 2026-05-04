Status: needs-triage

# 04 — Edges + cycle detection

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Add the task_edges table, the Graph engine module with cycle detection, and the `yojana_edge` MCP tool. Edges are typed relationships between tasks. The `depends_on` edge type creates real dependencies; the graph engine prevents cycles in the dependency subgraph.

- Migration 0003: task_edges table (id, source_task_id, target_task_id, edge_type, note, created_at)
- Store module: edge create, delete, list-by-task
- Graph engine module: cycle detection on `depends_on` edges (DFS-based). Other edge types (relates_to, supersedes, refines, motivated_by) are not checked for cycles.
- `yojana_edge` MCP tool with create, delete, list actions
- Edge uniqueness: (source, target, edge_type) is unique

## Acceptance criteria

- [ ] Edges are created between two tasks with a typed relationship
- [ ] Creating a `depends_on` edge that would form a cycle is rejected with a clear error
- [ ] Non-dependency edge types (relates_to, supersedes, refines, motivated_by) are not cycle-checked
- [ ] Deleting an edge works by edge ID
- [ ] Listing edges for a task returns both outgoing and incoming edges
- [ ] Cross-project edges are allowed (tasks in different projects can be linked)
- [ ] Graph engine is a pure module with no DB dependency, tested with unit tests for cycle detection (positive and negative cases, multi-hop cycles)
- [ ] Cascade delete: removing a task removes its edges

## Blocked by

- 02 — Task CRUD + sequence numbers
