# Handoff

## What's done

- All 8 v0 slices complete, committed, 71 tests passing
- 6 MCP tools: yojana_project, yojana_task, yojana_edge, yojana_query, yojana_ready, yojana_context
- 3 context shapes: summary, working, review
- mp-skills adapter docs in place
- Code review completed — 17 findings in docs/review-fixes-plan.md
- **Wave 1 + Wave 2 fixes applied** (11 items total)

## Wave 1 + 2 fixes completed

1. Tag filter: replaced LIKE with `json_each()` for exact match
2. Clear-to-NULL: `Option<Option<String>>` pattern on nullable TaskUpdates fields
3. Project status validation: rejects invalid status strings
4. Neighbor-loading helper: extracted in tools/context.rs
5. CancellationToken: `cancel.cancel()` called after graceful shutdown
6. SIGTERM handling: `tokio::select!` on ctrl_c + SIGTERM
7. JSON parse warnings: `tracing::warn!` on corrupt JSON fallbacks
8. Self-edge prevention: early check in `create_edge`
9. State machine transitions: added re-triage and in_progress→wontfix
10. Port env var warning: logs on invalid parse
11. Port-binding TOCTOU: bind directly, handle AddrInUse

## Pick up next

- **Wave 3 fixes** (scale prep, not urgent — defer until architecture changes)
- **First real use**: start yojana server, register the review findings as tasks in yojana itself (dog-fooding)
- **MCP config**: add yojana to manas MCP server configs so agents can use it in sessions

## Context needed

- Commit message workaround: panda breaks heredoc syntax. Use `printf ... > /tmp/file && git commit -F /tmp/file`
- `sutra_impact` must be called before editing load-bearing files (src/tools/task.rs has blast_radius=11)
- Db uses parking_lot::Mutex — public methods must not call other public methods (deadlock)
- Context assembler is pure — tool handler fetches data, assembler shapes it
- TaskUpdates nullable fields use `Option<Option<String>>`: None=keep, Some(None)=clear, Some(Some(v))=set
