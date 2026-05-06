# yojana

Local task graph for the manas ecosystem. SQLite-backed, exposed as an MCP server and a CLI. Tracks projects, tasks, dependencies, and contextual references for agent and human workflows.

## Install

```sh
cargo install --path .
systemctl --user restart yojana
```

Defaults: binary at `~/.cargo/bin/yojana`, DB at `~/.yojana/yojana.db`, MCP at `http://127.0.0.1:4200/mcp`.

## CLI

### `yojana serve [--stdio]`

Run the MCP server. Without `--stdio`, serves HTTP on the configured port. The systemd user unit invokes this; you rarely run it manually.

### `yojana projects [<slug>] [--all]`

List projects (default: `active` roots). `--all` includes paused/archived. With a slug, shows project detail and any nested workstreams.

### `yojana tasks <slug>[/N] [--status S] [--category C] [--all]`

List tasks for a project (and its descendant workstreams), or show one task by `slug/N`.

By default, terminal-status tasks (`done`, `wontfix`) are only shown if completed in the last 24 hours, grouped under a separate "Recently done" section. Use `--all` to include the full backlog. Passing `--status` disables the default-hide.

```
Active
 ID         Title              Status        Category
 myproj/3   Add foo            in-progress   enhancement
 myproj/5   Investigate bar    ready-for-agent  bug

Recently done (last 24h)
 ID         Title              Status   Category   Completed
 myproj/2   Wire baz           done     -          2026-05-06 09:14
```

The single-task detail view (`yojana tasks <slug/N>`) renders a Relationships block when the task has edges, showing them grouped by direction-aware label (`Blocks` / `Blocked by`, `Supersedes` / `Superseded by`, `Refines` / `Refined by`, `Motivated by` / `Motivates`, and the symmetric `Relates to`).

### `yojana todo <slug> "<title>" [-m "<body>"]`

Quickly capture a task in a project without an MCP/agent session. Creates the task with status `needs-triage` and prints its `slug/N`.

- The slug accepts the nested form (e.g. `chitta/research`).
- `-m "body"` populates the description.
- If `-m` is omitted and stdin is piped, stdin is read as the body.

```sh
yojana todo yojana "rethink list-view sort"
yojana todo chitta/research "test embedding compaction" -m "see notebook 2026-05-06"
git log -1 --pretty=%B | yojana todo yojana "follow up on last commit"
```

### `yojana done <slug/N> [--commit <sha>]`

Shorthand to mark a task done. With `--commit`, appends a `git:commit` context_ref recording the SHA the task shipped in. Multiple invocations with different SHAs accumulate.

```sh
yojana done yojana/9 --commit $(git rev-parse HEAD)
```

## MCP

All tools live under `mcp__yojana__*`. Highlights:

- `yojana_project` — create/get/update/list projects.
- `yojana_task` — create/get/update/comment on tasks. Supports a `commit` shorthand on `update` that appends a `git:commit` context_ref (so agents can record outcomes the same way as the CLI):
  ```json
  {"action": "update", "id": "yojana/9", "status": "done", "commit": "abc1234"}
  ```
- `yojana_query` — list tasks with filters. `include_all_terminal: true` disables the 24h done/wontfix hide; `recent_terminal_window_ms` overrides the window. `status: "done"` returns all done tasks regardless.
- `yojana_edge`, `yojana_ready`, `yojana_context` — dependency edges, ready-set queries, context-shape rollups.

## Status model

```
needs-triage ──► needs-info
             ──► ready-for-agent ──► in-progress ──► done
             ──► ready-for-human ─┘                 ──► wontfix
             ──► in-progress
             ──► wontfix
```

Terminal states (`done`, `wontfix`) reset to `needs-triage` if reopened. Transitioning into `done` records `completed_at`; transitioning out clears it.

## Storage

- DB: `~/.yojana/yojana.db` (SQLite, WAL).
- Migrations live in `migrations/`, applied automatically on startup.
- Each task carries a `history` JSON column with timestamped status transitions and field updates; `completed_at` is the denormalized cache of the most recent `→done` transition.
