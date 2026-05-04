Status: needs-triage

# 08 — mp-skills issue tracker adapter

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Create the `docs/agents/issue-tracker-yojana.md` template that maps mp-skills operations to yojana MCP tool calls, and update `docs/agents/issue-tracker.md` to point at yojana instead of local markdown.

This is the bridge that lets to-prd, to-issues, triage, and other mp-skills work against yojana natively. The adapter doc tells skills how to:
- Create an issue → `yojana_task action=create`
- List issues → `yojana_query`
- Apply a triage label → `yojana_task action=update` with status change
- Fetch a ticket → `yojana_task action=get` or `yojana_context`
- Publish a PRD → `yojana_project action=create` + `yojana_task action=create` for the PRD task

Also document the spike/experiment conventions: how to create a spike project, log experiments as tasks with category=experiment, and use the synthesis context shape (post-v0).

## Acceptance criteria

- [ ] `docs/agents/issue-tracker-yojana.md` exists with tool call mappings for all mp-skills operations
- [ ] `docs/agents/issue-tracker.md` updated to reference yojana as the active tracker
- [ ] Spike/experiment conventions documented
- [ ] A skill reading the adapter doc can create and query yojana tasks without ambiguity

## Blocked by

- 06 — Context shapes (summary + working)
