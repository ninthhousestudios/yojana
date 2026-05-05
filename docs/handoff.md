# Handoff

## What's done

- All 8 v0 slices complete, 75 tests passing
- 6 MCP tools: yojana_project, yojana_task, yojana_edge, yojana_query, yojana_ready, yojana_context
- 3 context shapes: summary, working, review
- mp-skills adapter docs in place
- **All 17 review findings resolved** (waves 1-3)
- Systemd user service running, MCP config in `~/.claude/settings.json`

## Review fixes summary

Waves 1+2: tag filter json_each, clear-to-NULL pattern, project status validation, neighbor-loading helper, cancel token, SIGTERM, JSON parse warnings, self-edge prevention, state transitions, env var warning, TOCTOU fix.

Wave 3: migration versioning (`_yojana_migrations` table), pagination (limit/offset on list_tasks and list_projects, default 100), `in_progress` renamed to `in-progress` with data migration.

Deferred (not needed yet): scoped cycle check (#13), scoped ready detection (#14), sequence number under pooling (#16).

## Pick up next

- **First real use**: register tasks in yojana itself (dog-fooding)
- **MCP config**: add yojana to other manas project `.claude/settings.json` files

## Context needed

- Commit message workaround: panda breaks heredoc syntax. Use `printf ... > /tmp/file && git commit -F /tmp/file`
- `sutra_impact` must be called before editing load-bearing files
- Db uses parking_lot::Mutex — public methods must not call other public methods (deadlock)
- Context assembler is pure — tool handler fetches data, assembler shapes it
- TaskUpdates nullable fields use `Option<Option<String>>`: None=keep, Some(None)=clear, Some(Some(v))=set
- Status is now `in-progress` (hyphenated), not `in_progress`
- Migrations are versioned — add new ones as `0006_*.sql` and register in the MIGRATIONS const
