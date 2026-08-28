# yojana

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

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

### `yojana task-edit <slug/N> [--title T] [-m DESC] [--status S] [--category C]`

Edit a task's title, description, status, or category. At least one flag is required.

Status changes from the CLI bypass the state machine — any valid status can be set from any other valid status. This lets you skip straight to `done`, reopen from `wontfix` to `in-progress`, etc. The MCP path still enforces the normal transition rules.

Use `--category=""` to clear the category.

```sh
yojana task-edit yojana/14 --title "revised title" -m "longer description here"
yojana task-edit yojana/14 --status done
yojana task-edit yojana/14 --status needs-triage --category bug
```

### `yojana done <slug/N> [--commit <sha>]`

Shorthand to mark a task done. With `--commit`, appends a `git:commit` context_ref recording the SHA the task shipped in. Multiple invocations with different SHAs accumulate.

```sh
yojana done yojana/9 --commit $(git rev-parse HEAD)
```

### `yojana tree <slug> [--all]`

Show the dependency tree for a project. Renders `depends_on` edges as an ASCII tree in execution order — roots are tasks with no dependencies (start here), children are tasks unlocked when the parent completes.

By default, entire trees where every task is terminal and completed >24h ago are hidden. `--all` shows the full history.

```
$ yojana tree chitta
chitta/1  [done] Migration 0007: external_refs typed column
└── chitta/4  [needs-triage] search_memories updates

chitta/2  [done] Migration 0008: soft-delete + retirement
└── chitta/4  (see above)

Standalone (no dependency edges):
  chitta/5  [needs-triage] chitta show CLI
```

Diamond nodes (depended on by multiple tasks) are shown once; subsequent appearances display `(see above)`.

## MCP

All tools live under `mcp__yojana__*`. Highlights:

- `yojana_project` — create/get/update/list projects.
- `yojana_task` — create/get/update/comment on tasks. Supports `arc_id` + `arc_phase` to assign a task to an arc phase, and a `commit` shorthand on `update` that appends a `git:commit` context_ref:
  ```json
  {"action": "update", "id": "yojana/9", "status": "done", "commit": "abc1234"}
  ```
- `yojana_arc` — create/get/update/advance/revert arcs. Create requires `project`, `title`, and `phases` (array of `{name, slice_type?, gate?}`). Advance moves to the next phase (or `skip=true` to mark skipped). Revert sets a completed phase back to active.
- `yojana_query` — list tasks with filters. Supports `arc` filter to scope by arc ID. `include_all_terminal: true` disables the 24h done/wontfix hide; `status: "done"` returns all done tasks regardless.
- `yojana_edge`, `yojana_ready`, `yojana_context` — dependency edges, ready-set queries, context-shape rollups. `yojana_ready` phase-gates arc tasks; `yojana_context` includes arc position for tasks in arcs.

## Status model

Valid statuses:

| Status | Meaning |
|---|---|
| `needs-triage` | Default on creation. Not yet sorted — scope unclear, priority undecided, or just freshly captured. |
| `needs-info` | Triaged, but blocked on information from a human (a question, a clarification, a decision). |
| `ready-for-agent` | Triaged and ready to be picked up by an agent (AFK). Acceptance criteria are concrete enough to execute against. |
| `ready-for-human` | Triaged and ready, but requires human attention (HITL — design decision, grilling, review). |
| `in-progress` | Actively being worked on. Cap yourself at a small number of these at a time. |
| `needs-review` | A task that is finished but needs to reviewed before being marked done. |
| `done` | Finished. `completed_at` is recorded. |
| `wontfix` | Closed without doing the work. Decision should be in `decisions` or a comment. |

Transitions:

```
needs-triage ──► needs-info
             ──► ready-for-agent ──► in-progress (──► needs-review) ──► done
             ──► ready-for-human ─┘                 ──► wontfix
             ──► in-progress
             ──► wontfix
```

`needs-info`, `ready-for-agent`, and `ready-for-human` can also transition back to `needs-triage` if scope shifts. `in-progress` can also step back to `needs-info` (work uncovered a blocking question) or `ready-for-human` (work needs human attention — design decision, review, grilling). Terminal states (`done`, `wontfix`) reset to `needs-triage` if reopened. Transitioning into `done` records `completed_at`; transitioning out clears it.

**Discipline note:** when you create tasks out of an explicit triage process (a review, a decompose, a planning session), set the status accurately on creation rather than letting `needs-triage` default. `needs-triage` means *untriaged*, not *just created*.

The MCP path enforces these transitions; the CLI (`yojana task-edit --status`) bypasses the state machine for ad-hoc fixups.

## Arcs

Arcs are lifecycle containers for tasks. A feature, bugfix, or spike flows through ordered phases — each phase groups related tasks and gates dispatch. Arcs live within a single project; cross-project work is tracked via task edges (`motivated_by`, `depends_on`).

**Key concepts:**

- **Phases** are defined inline at creation time (yojana validates shape, not vocabulary). Each phase has a `name`, optional `slice_type` (AFK/HITL), optional `gate` (auto/manual), and a `status` (active/pending/completed/skipped).
- The first phase defaults to `active`; the rest start `pending`. The current phase is derived (first non-completed, non-skipped phase).
- **Auto-gated** phases advance automatically when all their tasks reach terminal status (`done`/`wontfix`). Empty phases do not auto-advance.
- **Manual-gated** phases require explicit `advance` to proceed — useful as quality gates.
- **Phase revert** sets a completed phase back to active without cascading.
- **Arc status** (`active`/`paused`/`completed`/`abandoned`) is independent of phase progress. Pausing an arc stops all dispatch of its tasks without touching individual task statuses.
- `yojana_ready` only returns tasks whose arc phase is `active` and whose arc is `active`. Tasks not in any arc behave as before.
- `yojana_context` for a task includes its arc position and prior-phase decisions. Arcs also have a `summary` context shape.

**Identifiers:** arcs use a `~` sigil — `project/~N` (e.g. `yojana/~3`) to distinguish from task IDs.

**Example phase sets:**

| Arc type | Phases |
|---|---|
| Feature | design, decompose, implement, review, verify |
| Bugfix | diagnose, fix, verify |
| Spike | explore, synthesize, decide |



## Storage

- DB: `~/.yojana/yojana.db` (SQLite, WAL).
- Migrations live in `migrations/`, applied automatically on startup.
- Each task carries a `history` JSON column with timestamped status transitions and field updates; `completed_at` is the denormalized cache of the most recent `→done` transition.
