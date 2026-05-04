# yojana — design

Status: design captured, not started
Date: 2026-04-30
Source: design conversation comparing `~/soft/mymir` (Next.js project-management tool with native MCP) against the manas ecosystem, and `~/soft/mp-skills/skills/engineering/` (Matt Pocock's engineering skills) for process opinions.

---

## why yojana exists

The manas ecosystem has memory (chitta), code intelligence (sutra), filesystem indexing (smriti), and session coordination (sangha). It does not have a typed task graph. There is no answer to "where were we, and what should I work on next?" beyond grepping `.sessions/` and chitta.

Mymir solves that exact problem with a project graph that an agent walks into and reads from. We want the same primitive, built native to manas, composable with the existing services rather than collapsing them into one Postgres monolith.

The personal motivation: Josh's "where were we again?" is a real friction point that the current toolset does not address. Yojana makes that question answerable in one tool call.

## naming

**yojana** — Sanskrit for "plan, scheme, project." Slots cleanly next to chitta / smriti / sutra / sangha / kosha. Considered alternatives:

| candidate | meaning | rejected because |
|---|---|---|
| megha | cloud | evokes diffuse storage — opposite of "structured graph with dependencies" |
| vyuha | strategic formation | strong fit but more obscure |
| karya | work, task | names the leaves, not the whole |
| mandala | organized whole | overloaded culturally |
| tantra | system, framework | overlaps semantically with sutra |

Identifier prefix: **`YJN`**, e.g. `YJN-42`. Per-project sequence numbers, derived from a per-project slug. Stable and human-citable.

## design principle: grammar vs opinions

Yojana is **the grammar of work**, not the opinions about how work should be done.

- **Grammar (yojana)**: schema, identifiers, edge semantics, status state machine, context shape templates, dependency traversal, ready-detection. No opinion about *how* to brainstorm, decompose, plan, or execute.
- **Opinions (skills)**: brainstorming, decomposition, refinement, planning, execution, debugging, triage. Live as editable markdown skill files. The opinions evolve without schema migrations.

This is the same architecture mymir lands on: their MCP server is pure CRUD on the graph; their `agents/*.md` files carry the opinions. We borrow the structure, replace the opinions.

The opinions we adopt come from `mp-skills/skills/engineering/`. They are more mature than mymir's agents — a battle-tested vocabulary with an explicit bootstrap layer. We treat yojana as a new "issue tracker backend" that those skills already know how to plug into (mp-skills's `setup-matt-pocock-skills` already supports pluggable trackers — GitHub, GitLab, local markdown, "other"; yojana becomes the fourth option).

## architecture

Single-binary MCP server in Rust. Matches the sutra/smriti/sangha stack.

- **Storage**: SQLite, local-first. One DB per "yojana root" (typically `~/.yojana/<project-slug>.db` or per-repo `.yojana/index.db` — to be decided).
- **Tool surface**: stdio MCP plus optional HTTP for a future web UI.
- **No web UI in v0.** The agent surface is the surface. UI is post-stable.

## schema (v1)

```sql
-- projects
CREATE TABLE projects (
  id           BLOB PRIMARY KEY,           -- uuid v7
  slug         TEXT NOT NULL UNIQUE,        -- "manas-core", "yojana", "aion"
  title        TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  status       TEXT NOT NULL DEFAULT 'active',  -- active, paused, archived
  history      TEXT NOT NULL DEFAULT '[]',  -- jsonb of {ts, kind, payload}
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

-- tasks
CREATE TABLE tasks (
  id                  BLOB PRIMARY KEY,
  project_id          BLOB NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  sequence_number     INTEGER NOT NULL,            -- per-project counter; "YJN-42" = (yojana, 42)
  title               TEXT NOT NULL,
  description         TEXT NOT NULL DEFAULT '',
  category            TEXT,                        -- 'bug' | 'enhancement' | NULL until triaged
  status              TEXT NOT NULL DEFAULT 'needs-triage',
  slice_type          TEXT,                        -- 'AFK' | 'HITL' | NULL until decomposed
  acceptance_criteria TEXT NOT NULL DEFAULT '[]',  -- jsonb of {id, text, done}
  decisions           TEXT NOT NULL DEFAULT '[]',  -- jsonb of {situation, decided, rationale, rejected}
  implementation_plan TEXT,
  execution_record    TEXT,
  reproduction        TEXT,                        -- bug only — how to reproduce
  root_cause          TEXT,                        -- bug only — confirmed cause after diagnosis
  context_refs        TEXT NOT NULL DEFAULT '[]',  -- jsonb of refs (see below)
  files               TEXT NOT NULL DEFAULT '[]',  -- jsonb of file paths
  tags                TEXT NOT NULL DEFAULT '[]',
  history             TEXT NOT NULL DEFAULT '[]',  -- status transitions, edits
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL,
  UNIQUE (project_id, sequence_number)
);

CREATE INDEX tasks_project_status_idx ON tasks (project_id, status);

-- edges
CREATE TABLE task_edges (
  id              BLOB PRIMARY KEY,
  source_task_id  BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  target_task_id  BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  edge_type       TEXT NOT NULL,
  note            TEXT NOT NULL DEFAULT '',
  created_at      INTEGER NOT NULL,
  UNIQUE (source_task_id, target_task_id, edge_type)
);

CREATE INDEX task_edges_source_idx ON task_edges (source_task_id);
CREATE INDEX task_edges_target_idx ON task_edges (target_task_id);

-- conversations (per-task chat, optional)
CREATE TABLE task_conversations (
  id          BLOB PRIMARY KEY,
  task_id     BLOB NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  messages    TEXT NOT NULL DEFAULT '[]',  -- jsonb
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE INDEX task_conversations_task_idx ON task_conversations (task_id);
```

### state machine (status)

Adopted verbatim from `mp-skills/triage`, plus execution states:

```
needs-triage → needs-info → ready-for-agent → in_progress → done
                          ↘ ready-for-human ↗
                          ↘ wontfix
```

- `needs-triage` — newly created, awaiting evaluation
- `needs-info` — waiting on more information from reporter
- `ready-for-agent` — fully specified; AFK-ready; an agent can pick it up with no human context
- `ready-for-human` — fully specified but needs human implementation (judgment, external access, manual testing)
- `in_progress` — being worked
- `done` — closed, shipped
- `wontfix` — will not be actioned

State transitions are recorded in `tasks.history` with timestamps. Audit log comes free.

### edge types

```
depends_on    — source cannot start until target is done
blocks        — source is blocking target (inverse view; usually represented as depends_on the other way)
relates_to    — soft connection, not a dependency
supersedes    — source replaces target (target should be marked wontfix)
refines       — source is a more detailed version of target
motivated_by  — source exists because of target (e.g. refactor motivated by a bug)
```

`motivated_by` is specific to the diagnose → improve-codebase-architecture handoff pattern: when a bug fix reveals an architectural issue, the spawned refactor task carries `motivated_by` back to the original bug. Captures the genealogy of "this refactor exists because that bug existed."

### context_refs

Cross-service pointers, jsonb array of typed records sharing the manas-wide ref shape (see `docs/manas-architecture.md` § cross-tier identity). **Not opaque strings.**

```jsonc
[
  {"type": "doc:path",      "value": "docs/adr/0007",        "as_of": 1714780000000},
  {"type": "chitta:memory", "value": "abc123def...",         "as_of": 1714780000000},
  {"type": "sutra:symbol",  "value": "my::module::Foo",      "as_of": 1714780000000},
  {"type": "smriti:hash",   "value": "<blake3>",             "as_of": 1714780000000},
  {"type": "smriti:path",   "value": "/path/to/file.rs",     "as_of": 1714780000000}
]
```

Allowlisted types: `smriti:hash`, `smriti:path`, `sutra:symbol`, `kosha:citation`, `yojana:task`, `chitta:memory`, `doc:path`. Path types are second-class — prefer `smriti:hash` or `sutra:symbol` when available so refs survive moves and renames.

Yojana validates the shape but does not resolve refs. Resolution happens in **manas-cli** (per principle 9 — every cross-tier compound op lives in manas-cli, never inside a subsystem server). Yojana's binary stays a pure task-graph server.

## tool surface (v0)

Six tools to start. Mirrors the smallest useful slice of mymir's six tools, named in our convention:

| tool | purpose |
|---|---|
| `yojana_project` | create / get / list / update projects |
| `yojana_task` | create / get / update tasks |
| `yojana_edge` | create / delete edges |
| `yojana_query` | list tasks with filters (status, tags, ready, blocked) |
| `yojana_context` | bundle context for a task in one of four shapes (see below) |
| `yojana_ready` | what tasks have all `depends_on` edges satisfied? |

Routing decisions inside each tool follow rmcp's pattern (sutra/sangha do the same): one Rust function per verb, the tool dispatches by `action` parameter.

### context shapes (the killer feature)

```
yojana_context --task=YJN-42 --shape={summary|working|planning|agent}
```

Each shape is U-shape ordered: highest-recall content at start *and* end, less critical material in the middle. Mymir's specific contribution; we respect it.

| shape | when used | bundled content |
|---|---|---|
| `summary` | quick lookup | title, status, slice_type, edge counts, last 1 history entry |
| `working` | refining or reviewing | acceptance_criteria, decisions, 1-hop neighbors, last N task-conversation messages, related chitta observations (resolved from `task_chitta` link) |
| `planning` | writing the implementation plan | project brief, **upstream execution_records (multi-hop)**, prereq tasks' decisions, downstream tasks' specs, context_refs resolved (ADRs, CONTEXT.md terms, sutra symbols inlined) |
| `agent` | actively coding | implementation_plan, full upstream execution context, file paths, acceptance_criteria, sutra symbol locations for `tasks.files`, related ADRs |

**Cross-service joins are where manas pays off vs mymir.** Mymir cannot pull symbol locations from a code index because it has none.

**Where the join happens:** `yojana_context` returns the *unresolved* bundle from yojana itself — task fields, edges, the typed `context_refs` array, and per-ref hints. The fan-out (`sutra_outline` for files in `tasks.files`, `search_memories` for chitta observations, ADR file reads, kosha citation lookups) lives in **manas-cli** as a compound tool. The agent calls one manas-cli tool; manas-cli orchestrates the cross-tier reads and assembles the U-shape result. Yojana's MCP surface stays the six pure tools described above.

The `agent` shape is the answer to "where were we" — agent opens, asks for the bundle, walks in with the plan, the upstream decisions, the file paths, the AC, no briefing required.

## process layer — mp-skills as opinions

We do not write our own brainstorm/decompose/onboarding/manage agents. We adopt mp-skills:

| lifecycle stage | mp-skill | what it produces |
|---|---|---|
| capture intent | `to-prd` | a PRD published as a yojana project + initial tasks |
| break down | `to-issues` | vertical-slice tasks with HITL/AFK tags and `depends_on` edges, dependency-ordered |
| sharpen | `grill-with-docs` | refined task spec; CONTEXT.md and ADR side-effects inline |
| onboard existing repo | `improve-codebase-architecture` | deepening opportunities → tasks tagged with depth/locality reasoning |
| route work | `triage` | state machine; agent-briefs for `ready-for-agent` tasks |
| execute (feature) | `tdd` | red-green-refactor against the task's acceptance criteria as test list |
| execute (bug) | `diagnose` | reproduction, ranked hypotheses, root cause, regression test |

**Adoption mechanism:** mp-skills's `setup-matt-pocock-skills` supports pluggable issue trackers (GitHub, GitLab, local-markdown, "other"). We add **yojana** as the fourth backend. Concrete deliverables:

1. `docs/agents/issue-tracker-yojana.md` — template that says "this repo uses yojana; here are the tool calls for create-issue, list-issues, label-as-X."
2. `setup-yojana-project` skill — analogous to `setup-matt-pocock-skills` but writes a yojana-flavored `docs/agents/` block. Asks: project root, project slug, single- vs multi-context domain docs, AGENTS.md vs CLAUDE.md.
3. Vendor mp-skills into our setup (symlink or copy under `~/.claude/commands-archive/`) so they're invocable as `/to-issues`, `/triage`, etc.

Result: we get MP's full vocabulary (vertical slices, HITL/AFK, deep modules, deletion test, depth/locality/leverage, the diagnose loop, the triage state machine) without writing our own equivalents.

### opinion: tdd applies to new behavior, not behavior-preserving refactors

Worth flagging in the adapter doc. The mp `tdd` skill is correct for new features. For behavior-preserving moves (e.g. sutra's `pipeline.rs` extraction, where the spec is "byte-identical output before and after"), the existing test suite is the regression net; new tests of the extracted module's interface are not required. This is implicit in mp's "tests verify behavior" framing but worth being explicit so future-Josh does not over-test refactor PRs.

## ADR conventions

Adopt mp-skills's ADR discipline across all manas repos.

- Per-repo `docs/adr/` directory, numbered files (`0001-...md`, `0002-...md`).
- Format: Status / Context / Decision / Consequences. One decision per file.
- Write only when all three are true: hard to reverse, surprising without context, real trade-off with live alternatives.
- Numbers never reused. Superseded ADRs get a new ADR; the old one's status changes to `superseded by NNNN`.

### backfill candidates

Existing decisions in chitta-refactor.md and sutra-refactor-plan.md that qualify as ADRs (worth retroactively writing once):

**chitta:**
- "postgres stays as a real install dep" (rejected: trait, embedded postgres)
- "memory_type → deployment-configured allowlist" (rejected: open text, hardcoded)
- "engine + server crate split" (rejected: monolith)
- "embedder sidecar" (rejected: per-process model loading)

**sutra:** TBD — review sutra-overall-refactor.md for load-bearing decisions.

**smriti:** likely 1-2 (the SIGBUS-related WAL/mmap decisions look like ADR shape).

**yojana:** the design captured here is itself ADR-shape, but lives in this single doc for now. Split into individual ADRs if any decision is later contested.

### CONTEXT.md

Per-repo `CONTEXT.md` for domain glossary. mp-skills's `grill-with-docs` discipline applies: terms get added lazily as they crystallize during grilling sessions, never batched. Domain terms only — not implementation detail.

For the manas umbrella, the question is open: do we want a top-level `CONTEXT-MAP.md` pointing to per-repo CONTEXT.md files (multi-context layout)? Probably yes, given each manas repo has its own bounded vocabulary. Defer until any of the repos actually grows a `CONTEXT.md`.

## storage decision: SQLite, not Postgres

Mymir uses Postgres because it's a hosted multi-user web app. We are local-first, single-user, per-machine. SQLite matches sutra/smriti/sangha. No reason to introduce Postgres in the manas ecosystem when the only Postgres consumer is chitta (and only for its embedded vector / FTS work).

Trade-off accepted: cross-machine collaboration on the same yojana DB is not a v0 feature. If/when needed, options are (a) hosted yojana service, (b) git-tracked SQLite (works for single-writer), (c) sync layer. Defer.

## issue tracker scope: local only

Yojana is the issue tracker. No GitHub mirror. No sync. No reconciliation logic.

Tasks reference each other by yojana ID (`YJN-42`); commits and PRs reference yojana IDs in their messages — one-way, harmless. Public visibility for chitta/sutra (which are public repos) is handled by the future yojana web UI, not by syncing to GitHub Issues.

## sequencing (executive summary)

Captured fully in `../docs/handoff.md`. Short version:

1. Sutra + smriti refactors execute the normal way (their plan docs are the management surface).
2. Yojana v0 starts in parallel — schema, six v0 tools, no context shapes yet.
3. Once v0 runs, seed it with the three refactor plans as tasks-with-edges. Dogfood the context shapes against real data. Iterate the shapes.
4. Continue refactor execution. Pull the bug fix in smriti Wave 2 (MCP scans ignore `~/.smritiignore`) out and ship it standalone first.
5. Reunion: chitta phases 1-2 (engine split, embedder sidecar) — first work *planned through yojana* rather than pre-written as a free-standing plan doc. Real test of the system.
6. From there: next-feature planning runs through yojana natively.

## open questions / deferred decisions

- **DB location.** `~/.yojana/<slug>.db` (per-user, central) or `<repo>/.yojana/index.db` (per-repo, sidecar)? Per-repo lets a repo carry its own task graph and is easier to back up; per-user makes cross-repo "what's next" trivial. Lean toward per-user with a project slug, since cross-repo work is the actual common case in manas.
- **Per-project `CONTEXT.md` adoption.** Adopt now or wait until first grilling session naturally produces a term?
- **Manas umbrella `CONTEXT-MAP.md`.** Defer until at least one repo has a CONTEXT.md.
- **Web UI.** Out of v0 scope. Revisit after the agent surface stabilizes.
- **Multi-user / hosted yojana.** Deferred. If/when needed, see storage decision section.
- **task_chitta link table or `context_refs` strings?** Currently designed as `context_refs` strings. Revisit if cross-service join queries become slow or awkward — at which point a real link table might earn its keep.

## what v0 ships

- Cargo workspace at `manas/yojana/`
- Single binary, `yojana serve` (stdio) and `yojana --version`
- Migration `0001_init.sql` with the schema above
- Six v0 tools: `yojana_project`, `yojana_task`, `yojana_edge`, `yojana_query`, `yojana_context`, `yojana_ready`
- `yojana_context` ships with `summary` and `working` shapes only; `planning` and `agent` follow once dogfooded against the refactor plans
- One end-to-end test: create project → create three tasks with edges → query ready → fetch context for a task

Estimated effort: 2-3 focused sessions for v0.
