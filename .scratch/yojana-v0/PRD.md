Status: needs-triage

# Yojana v0 — PRD

## Problem Statement

When returning to work on a manas project — or any project managed through the manas ecosystem — there is no structured answer to "where were we, and what should I work on next?" Context is scattered across chitta memories, session transcripts, and free-form docs. An agent starting a session has no way to receive a structured briefing. Work decomposition, dependency tracking, and triage all happen informally or not at all. When Josh wants a different model (Codex, OpenCode) to review work, there is no structured way to hand over review context. When running experimental spikes, there is no way to track what was tried, what worked, and what didn't — making synthesis at the end manual and lossy.

## Solution

Yojana is a local-first task graph server that gives the manas ecosystem a structured, queryable representation of work. Projects contain tasks; tasks have typed edges (dependencies, refinements, motivations); tasks move through a triage state machine. Context shapes bundle the right slice of task data for different activities — a quick status check, a planning session, an agent picking up implementation, a reviewer in another tool, or synthesizing spike results. The mp-skills vocabulary (to-prd, to-issues, triage, tdd, diagnose) plugs into yojana as its issue tracker backend, providing the process opinions that yojana deliberately omits.

Yojana runs as an HTTP daemon, per-user, multi-project — the same deployment pattern as smriti, sangha, and chitta. A single SQLite database at `~/.yojana/yojana.db` holds all projects. Cross-project queries ("what's ready across everything?") and cross-project edges work naturally.

## User Stories

1. As a developer returning to a project, I want to ask "what's ready to work on?" so that I resume productive work without re-reading transcripts or grepping chitta.
2. As an AI agent starting a session, I want to fetch a structured context bundle for my assigned task so that I can begin coding without a lengthy human briefing.
3. As a developer, I want to create a project with a slug and title so that work is organized by project boundary.
4. As a developer, I want to create tasks within a project so that work items are tracked with status, acceptance criteria, and metadata.
5. As a developer, I want each task to get a stable per-project sequence number (e.g. YJN-42) so that tasks are human-citable in commits, conversations, and docs.
6. As a developer, I want to add typed edges between tasks (depends_on, relates_to, supersedes, refines, motivated_by) so that relationships are explicit and queryable.
7. As a developer, I want tasks to move through a triage state machine (needs-triage → needs-info → ready-for-agent/ready-for-human → in_progress → done/wontfix) so that work enters a consistent evaluation flow.
8. As an AI agent, I want tasks tagged as AFK vs HITL so that I know which tasks I can pick up autonomously without human judgment.
9. As a developer, I want to query tasks by status, tags, project, and blocked/ready state so that I can get different views of the work.
10. As a developer, I want to see which tasks are ready — all `depends_on` edges satisfied — so that I know what can start now.
11. As a developer, I want the graph engine to detect dependency cycles so that I don't create unresolvable task orderings.
12. As a developer, I want tasks to carry acceptance criteria as structured data so that the TDD skill can use them as a test list.
13. As a developer creating a bug task, I want fields for reproduction steps and root cause so that the diagnose workflow has structured places to record findings.
14. As a developer, I want tasks to carry typed cross-service references (chitta memories, sutra symbols, smriti files, ADR paths, git commits, git ranges) so that context travels with the task.
15. As a developer, I want task history (status transitions, edits) recorded automatically so that I have an audit trail.
16. As a developer, I want a `summary` context shape that returns title, status, slice type, edge counts, and last history entry so that quick lookups are cheap.
17. As a developer, I want a `working` context shape that returns acceptance criteria, decisions, 1-hop neighbors, and recent conversation so that refinement and review have the right context.
18. As a developer using mp-skills, I want yojana to function as an issue tracker backend so that to-issues, triage, and to-prd create and query yojana tasks natively.
19. As a developer, I want per-task conversation threads so that discussion is attached to the task, not lost in session transcripts.
20. As a developer, I want to update task fields (description, acceptance criteria, decisions, implementation plan) incrementally so that tasks are refined over time without replacing the whole record.
21. As a developer, I want to delete edges when dependency relationships change so that the graph stays accurate as plans evolve.
22. As a developer, I want projects to have status (active, paused, archived) so that I can filter out completed or shelved work.
23. As a developer wanting code review from another model (Codex, OpenCode), I want a `review` context shape that bundles the relevant commits, acceptance criteria, implementation decisions, and ADRs so that a reviewer gets full context without manual briefing.
24. As a developer, I want tasks to reference git commits and ranges via context_refs so that review and audit context is linked to the task graph.
25. As a developer running an experimental spike, I want to track experiments as tasks with hypothesis, results, and learnings so that I can synthesize findings when the spike concludes.
26. As a developer finishing a spike, I want a `synthesis` context shape that bundles all experiment results and learnings for the spike project so that I can produce a coherent "here's what we learned and what to build."
27. As a developer, I want cross-project queries ("what's ready across all my projects?") so that I can prioritize work across the manas ecosystem, not just within one repo.
28. As a developer, I want cross-project edges (e.g. a refactor in sutra motivated_by a bug found while building yojana) so that work relationships aren't artificially bounded by project.

## Implementation Decisions

- **HTTP daemon, per-user, multi-project.** Same deployment pattern as smriti/sangha/chitta. Not stdio per-project like sutra. Rationale: the most valuable query is cross-project ("what's next across everything?"), and cross-project edges are a real use case (manas monorepo). Single SQLite DB at `~/.yojana/yojana.db`, projects as rows.
- **Single Rust binary.** `yojana serve` starts the HTTP daemon. Matches the manas stack.
- **Four internal modules:**
  - **Store** — repository layer. All SQLite access, migrations, JSON column serialization behind CRUD interfaces. Testable with in-memory SQLite.
  - **Graph engine** — dependency traversal, ready-detection, cycle detection, topological ordering. Pure functions over edge data, no I/O.
  - **State machine** — validates legal status transitions, produces history entries. Pure logic.
  - **Context assembler** — collects task + edges + refs, arranges into U-shaped output per shape. v0 ships `summary` and `working` shapes only.
- **Six MCP tools:** `yojana_project`, `yojana_task`, `yojana_edge`, `yojana_query`, `yojana_context`, `yojana_ready`. Each dispatches by `action` parameter (rmcp pattern).
- **ID scheme:** UUID v7 primary keys internally; per-project integer sequence numbers for human-facing identifiers (e.g. `YJN-42`).
- **Context refs** are typed JSON records. Allowlisted types: `smriti:hash`, `smriti:path`, `sutra:symbol`, `kosha:citation`, `yojana:task`, `chitta:memory`, `doc:path`, `git:commit`, `git:range`. Yojana validates shape but does not resolve refs — resolution is a manas-cli compound operation.
- **State machine** adopts mp-skills triage labels verbatim plus execution states: needs-triage, needs-info, ready-for-agent, ready-for-human, in_progress, done, wontfix.
- **Edge types:** depends_on, relates_to, supersedes, refines, motivated_by. `blocks` is the inverse view of `depends_on`, not stored separately.
- **Context shapes:** v0 ships `summary` and `working`. Post-v0 shapes: `planning`, `agent`, `review`, `synthesis`.
- **Spike/experiment tracking** requires no schema changes. Tasks use `category: experiment`; existing fields map naturally (description → hypothesis, acceptance_criteria → success criteria, execution_record → results, decisions → learnings). The spike workflow is a skill-layer opinion, not grammar.
- **Yojana is "the grammar of work"** — schema, state machine, traversal, context shapes. Process opinions (brainstorming, decomposition, triage, execution) live in mp-skills as editable markdown.

## Testing Decisions

- Good tests verify external behavior through module interfaces, not internal implementation details.
- **Store module:** tested against in-memory SQLite. Cover CRUD operations, JSON column round-tripping, migration correctness, uniqueness constraints, cascade deletes.
- **Graph engine:** tested with pure functions. Cover ready-detection with various edge topologies, cycle detection (positive and negative cases), multi-hop traversal for context assembly.
- **State machine:** covered through integration tests that create tasks and attempt valid/invalid transitions.
- **End-to-end test:** create project → create three tasks with edges → query ready → fetch summary context for a task. This is the minimum "it works" gate for v0.
- MCP tool dispatcher is thin routing — tested implicitly through end-to-end tests.

## Out of Scope

- Web UI (revisit after agent surface stabilizes)
- `planning`, `agent`, `review`, and `synthesis` context shapes (post-v0, after dogfooding summary/working)
- Cross-machine sync or hosted yojana
- GitHub/GitLab issue mirror or sync
- Cross-service ref resolution (lives in manas-cli)
- Multi-user collaboration
- Spike-specific skills (to-spike, log-experiment, synthesize) — skill-layer work, not yojana server work

## Further Notes

- The design doc (`docs/yojana-design.md`) remains the architecture record — naming rationale, schema SQL, ADR conventions, ecosystem sequencing. This PRD is the implementable layer on top of it.
- The `motivated_by` edge type specifically supports the diagnose → improve-codebase-architecture handoff pattern from mp-skills.
- The `review` context shape is the answer to "how do I get Codex/OpenCode to review this?" — the task carries the commits, AC, decisions, and ADRs; the shape bundles them for a reviewer who has no prior context.
- Spike tracking demonstrates the grammar-vs-opinions separation: yojana's schema handles experiments without changes; the workflow differences are entirely in the skill layer.
