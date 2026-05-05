# Handoff

## What's done

- Slices 01 (HTTP skeleton + project CRUD) and 02 (task CRUD + sequence numbers)
- Slice 03 (state machine) -- `src/state.rs` pure module, integrated into `Db::update_task`
- Slice 04 (edges + cycle detection) -- `migrations/0003_edges.sql`, `src/graph.rs`, edge CRUD, `yojana_edge` MCP tool
- Slice 05 (query + ready detection) -- `yojana_query` with filters + ready/blocked flags, `yojana_ready` shortcut tool
- 55 tests passing
- 5 MCP tools: yojana_project, yojana_task, yojana_edge, yojana_query, yojana_ready

## Pick up next

- **Slice 06 (context shapes)** -- `yojana_context` MCP tool, `summary` and `working` shapes
- **Slice 07 (e2e integration test)** -- create project, tasks, edges, query ready, fetch context
- **Slice 08 (mp-skills adapter)** -- wire mp-skills issue tracker to yojana backend

## Context needed

- `parking_lot::Mutex` on `Db.conn` -- Db public methods must not call other Db public methods (deadlock). Use free functions taking `&Connection` internally
- Edge tool resolves task identifiers to UUIDs before calling `db.create_edge` to avoid deadlock
- `list_tasks` builds SQL dynamically with positional params; tag filter uses LIKE on JSON string
- `list_depends_on_with_status` joins edges with tasks to get target status in one query
- Graph engine `is_ready`/`blocked_by` are pure functions over `(Uuid, Uuid, String)` tuples
