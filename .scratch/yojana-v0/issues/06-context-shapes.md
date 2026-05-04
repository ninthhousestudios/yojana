Status: done

# 06 — Context shapes (summary + working)

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Add the Context assembler module, the task_conversations table, and the `yojana_context` MCP tool with `summary` and `working` shapes. Each shape bundles different slices of task data for different activities.

- Migration 0004: task_conversations table (id, task_id, messages JSON, created_at, updated_at)
- Context assembler module: `bundle(task, shape) → ContextBundle`
- `summary` shape: title, status, slice_type, category, edge counts (in/out by type), last 1 history entry
- `working` shape: acceptance_criteria, decisions, 1-hop neighbor summaries (tasks connected by any edge), last N conversation messages, context_refs (unresolved — just the typed refs, not their content)
- `yojana_context` MCP tool with `shape` parameter
- Conversation append support via `yojana_task action=comment` or a dedicated action

## Acceptance criteria

- [x] `yojana_context shape=summary` returns a compact bundle with title, status, slice_type, category, edge counts, last history entry
- [x] `yojana_context shape=working` returns acceptance_criteria, decisions, 1-hop neighbors (as summaries), recent conversation messages, and unresolved context_refs
- [x] Working shape's neighbor summaries use the summary shape (recursive but bounded to 1 hop)
- [x] Conversations are appendable and retrievable per task
- [x] Context assembler is testable — takes task + graph data as input, produces shaped output
- [x] Unknown shape names return a clear error listing valid shapes

## Blocked by

- 05 — Query + ready detection
