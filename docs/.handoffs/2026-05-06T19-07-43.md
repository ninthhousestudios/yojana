# Handoff — 2026-05-06

## State of the tree

Working tree has uncommitted changes from this session — completed_at + CLI default-hide + `yojana done` + README. Build clean, 86 tests pass, 0.2.1 installed and the systemd service is running on it. Josh has not requested a commit yet; offer one when picking this back up.

Modified/new files:
- `migrations/0007_completed_at.sql` (new)
- `src/db.rs` — TaskRow.completed_at, TASK_SELECT, update_task transitions, TaskQueryFilter::include_terminal_after, TERMINAL_STATUSES
- `src/main.rs` — `Tasks --all`, partition into Active / Recently done, new `Done` subcommand
- `src/display.rs` — Completed column when any row has completed_at
- `src/tools/task.rs` — TaskOutput.completed_at, TaskArgs.commit shorthand
- `src/tools/query.rs` — include_all_terminal, recent_terminal_window_ms
- `src/context.rs` — test fixture only
- `Cargo.toml` — 0.2.1
- `README.md` (new)

## What to pick up

- **Commit the session's changes.** No commit was made; everything is in the working tree. Suggested message: `feat: completed_at tracking, CLI default-hide done>24h, yojana done with --commit`.
- **Optional follow-ups Josh raised but didn't ask for:**
  - Post-commit git hook that auto-links `yojana:<slug>` mentions in commit messages → adds `git:commit` ref to the task. Plan only — implement if Josh wants the ergonomics.
  - `snoozed_until` field for dated deferral. Only implement if a concrete "wait until date X" use case shows up.

## Context the next session needs

- Decisions are in chitta — search `tags:decision yojana 2026-05-06` for: rejection of `cancelled`/`deferred` statuses; rationale for `completed_at` column + `git:commit` context_ref over a dedicated `task_commits` table.
- The 24h default-hide is wired via `TaskQueryFilter::include_terminal_after`. Both the CLI Tasks command and the MCP `yojana_query` tool default to a 24h window; passing `--status` (CLI) or a `status` filter (MCP) disables it; `--all` / `include_all_terminal: true` disables it explicitly.
- The MCP `yojana_task` action=update now accepts a top-level `commit: <sha>` field. It merges into context_refs (preserving any explicit context_refs the caller also passed, plus existing refs on the task if no list was passed).

## Blockers

None.
