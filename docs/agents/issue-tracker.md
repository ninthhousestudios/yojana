# Issue tracker: Active backend

The active issue tracker for this repo is **yojana**. See `docs/agents/issue-tracker-yojana.md` for all tool call mappings.

Legacy issues from before yojana may still exist as markdown files in `.scratch/`. These are read-only reference — new work goes through yojana.

## Quick reference

| Operation | Tool call |
|---|---|
| Create issue | `yojana_task action=create project="<slug>" title="..."` |
| Get issue | `yojana_task action=get id="<slug>/<N>"` |
| List/query | `yojana_query project="<slug>" status="..." tag="..."` |
| Triage | `yojana_task action=update id="<slug>/<N>" status="<label>"` |
| Ready tasks | `yojana_ready project="<slug>"` |
| Context bundle | `yojana_context task="<slug>/<N>" shape="summary\|working"` |
| Comment | `yojana_task action=comment id="<slug>/<N>" text="..."` |
