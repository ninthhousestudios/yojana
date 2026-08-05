# Seeds comparison: what yojana can learn

[Seeds](https://github.com/jayminwest/seeds) (`sd`) is a git-native issue
tracker for AI-agent workflows — the successor to beads in the overstory/mulch
ecosystem. Like yojana it targets the human + agent workflow, tracks
dependencies and lifecycle, and treats agent ergonomics as a first-class
concern. Unlike yojana it is **more mature as a plain tracker**: richer
decomposition, labels, search, health tooling, and a broad agent-facing output
surface.

This document is the seeds counterpart to `beads-comparison.md`. It captures
what's worth borrowing — prioritized for a yojana that's already good and
doesn't need a rewrite — plus an honest account of where yojana already leads
and what to leave on the table.

Seeds references are to files in `~/soft/seeds/src`. Yojana references are to
`~/soft/manas/yojana/src`.

## Architectural divergence

The beads comparison diverged on *who the actor is* (solo human vs. swarm of
agents). The seeds comparison diverges on *where the data lives*.

**Yojana is a service.** A SQLite DB at `~/.yojana/yojana.db`, fronted by a
long-running MCP server (`serve_http`, `src/main.rs`) and a thin CLI. State is
central, single-writer, and lives outside any repo. Migrations run on startup
(`migrations/`). This is a deliberate, coherent design: one operator, one
graph, transactional integrity, rich queries over indexed columns.

**Seeds is a file.** `.seeds/issues.jsonl` committed inside the repo it tracks,
one JSON object per line, with a `merge=union` gitattribute and dedup-on-read
(last occurrence wins) so parallel branches merge without a sync step. There is
no service and no separate database — "the JSONL file IS the database." State
travels with the code, diffs in review, and branches with the work.

Neither is strictly better; they optimize different things. That divergence is
worth a full discussion (see *Storage philosophy* below), but it is **not** the
main payload of this document. Most of what yojana can learn from seeds is
independent of storage — features and ergonomics that would slot into the
SQLite model unchanged.

## Data model comparison

| Concept | Yojana | Seeds |
|---------|--------|-------|
| Work unit | Task, `project/N` seq + UUID | Issue, `{project}-{4hex}` |
| Status | 8 states, enforced transition allow-list (`src/state.rs`) | 3 states: `open`/`in_progress`/`closed` |
| Dependencies | 5 typed edges: `depends_on`, `relates_to`, `supersedes`, `refines`, `motivated_by` (`src/tools/edge.rs`) | `blocks`/`blockedBy` only (`src/commands/dep.ts`, `block.ts`) |
| Lifecycle | Arcs: ordered phases, auto/manual gates (`src/tools/arc.rs`) | Plans: templated decomposition into child issues (`src/commands/plan.ts`) |
| Tagging | `category` free-text (bug/enhancement/experiment) | `category` **and** free-form labels (`src/commands/label.ts`) |
| Search | Structured filters only (`yojana_query`) | Structured filters **and** full-text (`src/commands/search.ts`) |
| Audit | `history` JSON column on task row | closeReason + timestamps; no rich event log |
| Cross-tool refs | `context_refs` typed pointers (`sutra:symbol`, `git:commit`) | `extensions` opaque namespaced blob + plan/mulch links |
| Health | — | `sd doctor [--fix]` (`src/commands/doctor.ts`) |
| Agent bootstrap | `yojana_context` shapes, `CONTEXT.md` | `sd prime`, `sd onboard`, `sd completions` |
| Output | MCP JSON; CLI human tables | `--json` everywhere + `--format json\|compact\|markdown\|plain\|ids` |

## Where yojana already leads

State this plainly so the rest of the document is trustworthy. Yojana is ahead
of seeds on the *semantic* axis:

- **Status model.** Yojana's 8 states with an enforced transition machine
  (`src/state.rs`) encode workflow discipline — `needs-triage`, `needs-info`,
  `ready-for-agent`/`ready-for-human` (AFK/HITL split), `needs-review`. Seeds
  has three states and pushes all nuance into labels and plan structure. Do not
  regress toward that.
- **Typed edges.** Five relationship types vs. seeds' single blocking
  dependency. `supersedes`/`refines`/`motivated_by` carry intent that seeds
  can't express.
- **Arcs.** Phase gating (auto/manual), phase revert, arc-level pause, and
  `yojana_ready` phase-gating are a real lifecycle engine. Seeds plans are a
  *decomposition* tool, not a gating one (see below) — they overlap but don't
  subsume.
- **Context shapes.** `yojana_context` assembling mode-specific rollups is more
  sophisticated than anything in seeds' `sd prime`.

The lessons below are about breadth (features seeds ships that yojana lacks),
not about yojana's core model, which is stronger.

## Adoptable now

Ordered by value-to-effort for a solo HITL operator.

### 1. Plans — structured decomposition (highest value)

The beads comparison flagged "structured multi-step workflows" as gap #7 and
left it as an open design question ("do arcs subsume molecule semantics?").
Seeds answers that question with a shipped, mature subsystem worth studying
before yojana designs its own.

**What seeds does** (`src/commands/plan.ts` — the single largest command at
~86KB, `src/plan-schema.ts`, `src/plan-templates-loader.ts`):

- Named templates — built-in `feature` / `bug` / `refactor`, plus custom — each
  a blueprint of steps (`sd plan templates`).
- A two-phase flow: `sd plan prompt <seed>` emits a structured JSON skeleton the
  LLM fills in; `sd plan submit <seed> --plan <file>` validates it against the
  template schema and **spawns one child issue per step**, wiring
  `step.blocks` into real `blockedBy` dependencies.
- Adopt-only plans for "release trains": `sd plan create` → `sd plan adopt` →
  `sd plan reorder` to pin an exact serial order over pre-existing issues
  (`src/commands/plan-adopt-only.test.ts`).
- Lifecycle: `plan show/validate/list/edit/outcome/review`, revision bumping on
  structural change, and `seeds:plan-backref` blocks kept in sync on child
  issues (`src/plan-backref.ts`).
- Cross-tool enrichment: plans pull `prior_art` from mulch domains and can
  record the chosen approach back as a mulch decision (`src/plan-mulch.ts`,
  `src/plan-domain.ts`).

**How this maps to yojana.** Arcs already own *lifecycle phases*; plans own
*decomposition into a dependency-wired task set*. These are complementary, not
competing — the seeds design is evidence you can have both. A yojana
`yojana_plan` (or an arc extension) that takes a template + an LLM-filled
skeleton and emits child tasks with `depends_on` edges pre-wired would give
yojana the one thing arcs don't: automated fan-out from a decomposition. The
`prompt → fill → submit` handshake is the transferable idea — it turns "decompose
this" from a freeform agent action into a validated, reproducible operation.

Note the strongest transferable detail: **steps carry `labels[]` that flow to
spawned children**, so agent-generated tasks are tagged at birth (e.g.
`"labels": ["nightwatch"]`) with no post-hoc labelling pass. That depends on
labels existing — see #2.

**Effort:** High. This is a subsystem, not a field. But it's the highest-value
thing seeds has that yojana lacks, and yojana's typed edges + arcs are a better
substrate to build it on than seeds' flat blocking model.

### 2. Labels — a flat tagging dimension

**Gap:** Yojana has exactly one categorical axis, `category` (free-text
`Option<String>`, `src/db.rs`). There is no way to attach multiple orthogonal
tags to a task. Confirmed: `label` in yojana source refers only to
`context_ref` labels and edge-display strings, never task tags.

**Seeds mechanism:** `sd label add/remove/list/list-all`
(`src/commands/label.ts`). Labels normalize (lowercase, trim, dedupe), filter
the ready/list/search sets (`--label`, `--label-any`, `--unlabeled`,
`src/filter.ts`), and — critically — integrate with plans (spawned children
inherit step labels additively without clobbering hand-added ones).

**Implementation:** A `task_labels(task_id, label)` join table (or a JSON array
column mirroring `history`), `yojana_task` accepting a `labels` field,
`yojana_query` gaining `label`/`label_any`/`unlabeled` filters. Low effort,
high daily utility — this is the cheapest item here and probably the first one
to ship.

### 3. Full-text search

**Gap:** Yojana has no search. `yojana_query` filters by status/category/arc but
you cannot find a task by a word in its title or body. For a backlog past a few
dozen tasks this bites.

**Seeds mechanism:** `sd search <query>` — substring match over title +
description, composable with every list filter and `--format`
(`src/commands/search.ts`).

**Implementation:** SQLite makes this nearly free — an FTS5 virtual table over
`title`/`description`, or even a `LIKE` scan at yojana's data scale. Expose as
`yojana_query`'s `search` param or a dedicated `yojana search`. Low effort.

### 4. Doctor — integrity checks

**Gap:** As the task graph grows, invariants drift: edges pointing at deleted
tasks, arcs whose current-phase derivation breaks, tasks in `done` with a live
`depends_on`, malformed `history` JSON, orphaned workstreams. Yojana has no
tool to detect or repair these; the only `health` in the codebase is the HTTP
liveness endpoint (`/health`).

**Seeds mechanism:** `sd doctor [--fix]` (`src/commands/doctor.ts`, ~27KB) — a
battery of named checks, each with a pass/fail count and an auto-fixer where
safe (e.g. the `extensions-schema` check drops non-object values under `--fix`).

**Implementation:** A `yojana doctor` walking the invariants yojana already
implies — edge referential integrity, status/transition legality for
CLI-bypassed edits (`task-edit` deliberately skips the state machine, so it's
the prime source of illegal states), arc phase consistency, `completed_at` vs.
status agreement. Medium effort, and it pays for itself the first time a
`task-edit --status` fixup leaves the graph inconsistent.

### 5. Output format matrix

**Gap:** Yojana's CLI emits human tables (`src/display.rs`); the MCP path emits
JSON. There's no `--json` on the CLI and no compact/id-only modes for piping.

**Seeds mechanism:** Every command takes `--json`, plus
`--format json|compact|markdown|plain|ids`. `ids` in particular is the
xargs-friendly primitive that makes shell composition trivial
(`sd ready --format ids | xargs -n1 sd show`).

**Implementation:** A `--format` flag threaded through `display.rs`. Low effort,
and it makes the CLI scriptable for the ad-hoc AFK flows the MCP server isn't
always running for.

### 6. Session bootstrap & shell UX (`prime` / `onboard` / `completions`)

**Partial gap.** Yojana's `yojana_context` shapes and `CONTEXT.md` already cover
much of what seeds' `sd prime` does (inject session rules + command reference).
But seeds also ships:

- `sd onboard` (`src/commands/onboard.ts`) — idempotently writes a marker-
  delimited seeds section into `CLAUDE.md`/`AGENTS.md` so a fresh repo teaches
  the agent about the tracker automatically. Yojana's manas-instructions block
  is injected by manas tooling; a self-contained `yojana onboard` would make
  yojana usable outside manas.
- `sd completions <bash|zsh|fish>` (`src/commands/completions.ts`) — clap makes
  this ~10 lines via `clap_complete`. Pure DX, near-zero effort.

**Effort:** Low. `completions` is trivial; `onboard` is a nice-to-have that only
matters if yojana is meant to stand alone.

### 7. Extensions — opaque consumer-owned metadata

**Gap / design note.** Yojana's `context_refs` are *typed* cross-tool pointers —
great for "this task relates to `sutra:symbol foo`." What yojana lacks is a
place for a downstream consumer to stash **opaque, namespaced runtime state**
against a task without a schema change.

**Seeds mechanism:** `Issue.extensions?: Record<string, unknown>`
(`src/types.ts`) — explicitly opaque to seeds, owned by consumers (warren,
greenhouse, overstory) under namespaced keys. `sd update --extensions <json>`
shallow-merges; two well-known keys (`queued`, `scheduledFor`) are read by
`sd ready --respect-schedule` and nothing else.

**Why yojana might want this.** If anything in the manas ecosystem ever needs to
annotate yojana tasks with its own state (a dispatcher's `lastRunId`, a
scheduler's deferral), `context_refs` is the wrong shape and a schema migration
per consumer is friction. An `extensions` JSON column gives ecosystem tools a
sanctioned scratch space. **Effort:** Low (one column) — but only worth it once
a second tool actually needs to write to yojana. Don't build it speculatively.

### 8. Scheduling / deferred work in the ready set

**Gap:** `yojana_ready` returns everything unblocked and phase-active. There's
no notion of "not yet" — a task deferred to next week still shows as ready.

**Seeds mechanism:** `sd ready --respect-schedule` (opt-in) honors
`extensions.queued === true` and a future `extensions.scheduledFor` ISO
timestamp, parking those tasks out of the ready set (`src/filter.ts`). Default
ready is byte-identical to the unscheduled behavior.

**Implementation:** Rides on #7. A `scheduled_for` column (or extensions key) +
an opt-in `respect_schedule` flag on `yojana_ready`. Low effort, and it's the
HITL-friendly half of the "gates" idea the beads doc wanted — a way to say "not
now" without a fake blocking edge.

## Storage philosophy: JSONL vs. SQLite

This is the one place the divergence is strategic rather than a feature to bolt
on. But the framing that matters is this: **storage is downstream of scope.**
Neither project chose a storage engine and then built a tracker. Each chose a
*scope*, and the storage fell out of it as a forced consequence. Comparing the
engines in the abstract — "is a diffable text file better than an indexed DB?" —
is the wrong question and yields a false winner.

### The two engines, dimension by dimension

| Dimension | Seeds — JSONL in-repo | Yojana — central SQLite |
|---|---|---|
| Write model | Advisory lock (`O_CREAT\|O_EXCL`) + temp→rename; mutations **rewrite the whole file** | WAL, single-writer, `BEGIN IMMEDIATE`; multi-row/column atomic txns |
| Write cost | O(file size) — every edit rewrites all issues | O(change) — one row |
| Read cost | O(n) full scan + parse every line | Indexed; joins, aggregates, FTS5 |
| Graph scope | **Per-repo** — no cross-repo query exists | **Global** — nested projects, cross-project edges |
| Git behavior | Data *is* versioned (`merge=union` + dedup-on-read) | Data *not* versioned; DB at `~/.yojana`, outside git |
| Merge | Auto-*suppresses* conflicts; last-in-file wins | No merge — one central timeline |
| Reviewability | Issue changes show in `git diff` / PR | Opaque; history only via yojana |
| Infra | Zero — `git clone` and it's there | systemd service + MCP port |
| Durability | Crash-safe swap, but logical drift possible → needs `doctor` | ACID + constraints prevent most drift |
| Survives tool death | Yes — `jq`-readable text | Harder — DB + a dead server to exhume |

### Storage is forced by scope

**Seeds' scope is per-repo, branch-parallel, agent-collaborative.** Once you
decide issues belong *inside the repo they describe*, everything is forced: it
must be a text file (so git versions it), it must union-merge (so parallel
branches don't block), it must be diffable (so PRs show issue changes), and it
must be zero-infra (so `git clone` just works). JSONL isn't a preference — it's
the only thing satisfying those constraints. The weaknesses follow just as
directly: no cross-repo graph (data is trapped per repo), full-scan reads (no
index in a flat file), and the *need* for `sd doctor` (nothing enforces
referential integrity, so drift is swept up after the fact).

**Yojana's scope is cross-project, single-operator, one unified graph.** Once
you decide the operator reasons over *one task graph spanning every project and
workstream*, that too is forced: the data can't live in any single repo (it
spans them), it needs indexed queries and joins (the graph *is* the product),
and cross-project edges (`motivated_by`, `depends_on`) need referential
integrity a flat file can't give. Central SQLite is the only thing satisfying
*those* constraints. Its weaknesses follow: no branch-local issue state, opaque
non-reviewable history, and a service that has to be running.

So the choices are **not interchangeable options** — they're load-bearing
consequences of two different bets about what the tracker is *for*. You cannot
cherry-pick. Give yojana seeds' branch-parallel reviewable issues and you must
push data back into repos, losing the unified graph that *is* yojana. Give
seeds yojana's cross-project graph and you must centralize storage, reinventing
SQLite-in-a-textfile badly.

### The merge story is conflict-*suppression*, not resolution

The most oversold part of the git-native pitch. `merge=union` does not *merge*
two edits — it concatenates both sides, and dedup-on-read keeps the last
occurrence *by file position, not `updatedAt`*. If branch A closes issue `X`
and branch B retitles the same `X`, you do not get a closed-and-retitled `X`;
you get two `X` lines and one wins wholesale — possibly the staler one. One
side's edit is silently discarded.

For seeds' domain this is the right trade: issues are usually touched on one
branch at a time, a silent last-writer-wins beats a *blocking* merge conflict
(halting an agent mid-flow is expensive, issue metadata is cheap to
reconstruct), and it never stalls a merge. But name it precisely: seeds trades
*correctness under concurrent same-issue edits* for *never blocking*. Yojana,
single-writer by construction, simply never has this collision — it buys that
immunity with the central-service constraint. The conflict is dodged, not
solved; different scope, different problem.

### Where they could converge: export, not migration

The git-native benefits split into two piles, and only one needs the JSONL
*architecture*:

- **Write-side** — branch-local issue state, conflict-free parallel writes.
  These need the whole git-native model. Yojana, single-operator, needs these
  **least**.
- **Read-side** — diffable/reviewable history, `jq`-able portability,
  survives-tool-death. These are about *serialization*, not the write path, and
  can be had without changing how yojana writes.

Hence the only worthwhile move: **export, not migration.** A `yojana export`
that derives a per-project JSONL snapshot into each repo (committable, diffable,
`jq`-able) while SQLite stays the source of truth. You harvest the read-side
pile and keep the query engine and the unified graph, forfeiting only the
write-side pile — the pile yojana has least use for.

A full migration to git-native storage would be strictly wrong for yojana: it
sacrifices the cross-project graph that justifies yojana's existence to gain
branch-parallel writes it doesn't need. Export is a weekend feature that
harvests the real benefits; migration is a rewrite that trades your best asset
for your least-needed one. Do it only if reviewable/portable issue history
becomes a *felt* need rather than a nice idea.

### What export is actually for

The motivating pain is concrete: git already carries `yojana/N` references in
commit messages, but they resolve to nothing for anyone without the author's
local DB. Export closes that gap — but "export" is really three distinct use
cases sharing one serializer, differing in *granularity*, *trigger*, and *what
they contain*. Full design detail lives in the yojana task; the framing:

1. **Reference resolvability (the actual pain).** A lightweight *manifest* —
   `id → {title, status, closedAt}`, one deterministic line per task —
   committed in-repo and refreshed on the normal push cadence, covering *all*
   tasks regardless of state. A reader hits `yojana/9`, greps one file, gets
   "Add foo — done." This alone solves the stated problem. The trigger is
   **push cadence, not task close**: a reference must resolve while the task is
   still open (PR review) and even if it never closes (`wontfix`, abandoned) —
   both of which a close-triggered export would miss.

2. **Task provenance (archival bonus).** The full record — description,
   history, edges, decisions — written once when a task reaches terminal state.
   A closed task is frozen, so close *is* the right trigger here (unlike case
   1). Optional second layer for when you want the "why," not just the "what."

3. **Project archival (finished projects).** A one-shot dump of a completed
   project's entire task graph into its repo at project end. The cleanest case:
   the project is dead, so no churn, no merge surface, no cadence — export once,
   commit, done. The repo then carries not just its git history but the
   decomposition, arcs, decisions, and wontfix rationale behind it, as portable
   `jq`-able text that outlives yojana. It doubles as DB hygiene: a natural
   boundary to set the project `archived` (or eventually retire it from the live
   graph) with the record preserved — the pragmatic answer to the "compaction"
   problem the beads comparison waved off, with no tiered-snapshot machinery.
   Worth it *when the task graph carries rationale git doesn't* (arcs,
   decisions, edges); redundant with `git log` for a project that was a flat
   mechanical todo list — so opt-in per project, not automatic.

Two costs the design must handle regardless of case:

- **Project↔repo binding doesn't exist.** `ProjectRow` is
  slug/title/status/parent — no repo path. `yojana export` in a repo must be
  told which project(s) map to it (a `.yojana/export.toml` naming slugs, or a
  `repo_path` on the project). First thing to solve; everything depends on it.
- **The export file is itself a merge surface** (cases 1–2, not 3). The irony of
  the whole JSONL-vs-SQLite discussion: a continuously-refreshed committed file
  conflicts across branches. Make it deterministic — strict id sort, stable
  field order — and consider seeds' own `merge=union` gitattribute on it, so the
  resolvability fix doesn't reintroduce the merge pain SQLite let yojana avoid.

## What's not worth adopting

- **The JSONL storage rewrite.** Covered above — yojana's cross-project graph
  and MCP-service model are load-bearing; git-native storage would trade them
  away for portability yojana doesn't currently need. Export bridges the gap.
- **Dedup-on-read / `merge=union`.** These exist to make a git-mergeable file
  survive parallel branch writes. SQLite's single writer has no such problem;
  adopting them would be solving a problem yojana doesn't have (the mirror of
  the beads doc's "row lock" verdict).
- **`sd config` schema wire-surface.** Seeds' schema-driven `sd config`
  (`src/config-schema.ts`) exists specifically as the wire surface for warren's
  config editor. Yojana is *already* an MCP service — a config UI would talk to
  it directly, not through a CLI schema shim. Yojana's env-var `Config`
  (`src/config.rs`) is right-sized.
- **`sd upgrade` / npm machinery.** Seeds self-updates from npm because it's a
  distributed Bun package. Yojana ships via `cargo install --path .` +
  systemd; `git pull && cargo install` is the idiom and needs no in-tool
  command.
- **Collapsing yojana's status model toward seeds' three states.** Explicitly
  don't. Yojana's 8-state machine is an asset; seeds' minimalism is a
  constraint it works around with labels and plans, not a target.

## Suggested roadmap ordering

Cheap-and-high-value first, structural bets last:

1. **Labels** (#2) — low effort, immediate daily utility, unblocks plan-label
   inheritance later.
2. **Full-text search** (#3) — low effort, FTS5 is nearly free at yojana scale.
3. **`--format` on the CLI + `completions`** (#5, #6) — low effort, makes the
   AFK/scripting path first-class.
4. **`doctor`** (#4) — medium effort; the safety net for `task-edit`'s
   deliberate state-machine bypass.
5. **Scheduling** (#8) + **extensions** (#7) — pair them; the HITL "not now"
   primitive plus its storage substrate. Build extensions only when a second
   tool needs to write to yojana.
6. **Plans / decomposition** (#1) — the big one. Design it as a *complement* to
   arcs (decomposition + edge fan-out), not a replacement, and steal the
   `prompt → fill → submit` handshake wholesale.
7. **Git-native export** (storage section) — only if reviewable/portable issue
   history becomes a felt need. Export, never migrate.
