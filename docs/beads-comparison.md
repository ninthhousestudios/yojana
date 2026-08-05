# Beads comparison: what yojana can learn

Beads is a task/issue tracker built for long-horizon, multi-agent autonomous
work. Yojana is built for human-in-the-loop (HITL) solo operation. This
document captures concrete architectural differences and what's worth
adopting — now for HITL use, and later if yojana grows toward autonomous
agent support.

## Architectural divergence

Both systems track work state, dependencies, and lifecycle. They diverge on
**who the actor is**.

Yojana assumes a single human operator. The state machine enforces discipline,
context shapes serve different cognitive modes, and arcs give lifecycle
structure. There is no concept of *who holds the work* because the answer is
always the operator.

Beads assumes multiple autonomous agents that may be unreliable, concurrent,
and unsupervised. It builds an entire coordination layer yojana lacks:
claims with lease expiry, heartbeats, a reaper for stuck work, row-lock-forced
serialization to surface silent merge conflicts, and compare-and-set guards
for safe state transitions. The storage choice follows: Dolt gives cell-level
3-way merge for distributed writers; yojana's single-connection SQLite is
adequate for one human.

## Data model comparison

### Shared concepts

| Concept | Yojana | Beads |
|---------|--------|-------|
| Work unit | Task (UUID, project-scoped sequence number) | Issue ("bead") with content hash for dedup |
| Dependencies | `task_edges` with typed edges (`depends_on`, `relates_to`, `supersedes`, `refines`, `motivated_by`) | `dependencies` table with types (`blocks`, `parent-child`, `conditional-blocks`, `waits-for`) |
| Lifecycle phases | Arcs with ordered phases, auto/manual gates | Molecules (epic parent + dependency-sequenced children) |
| Status machine | 8 states, explicit transition allow-list | Simpler open/in_progress/closed with claim overlay |
| Audit trail | `history` JSON array on task row | Separate `events` table |
| Cross-tool refs | `context_refs` (typed pointers: `sutra:symbol`, `git:commit`, etc.) | `ExternalRef`/`SourceSystem` for federation |

### Beads-only concepts

**Claims and leases.** Assignment is a plain string field. Claiming is atomic
(first-writer-wins, idempotent for current holder). A separate `leases` table
(Dolt-ignored so heartbeats don't spam version history) tracks `holder`,
`granted_at`, `lease_expires_at`, `heartbeat_at`. A reaper (`bd reclaim`)
scans expired leases and reverts stranded work to `open`.

**Row lock.** A `row_lock` BIGINT column forces every mutating write path to
touch the same cell. Since Dolt does cell-level merge, two concurrent writers
touching different columns would silently merge; `row_lock` converts that
into a real serialization conflict that retry logic handles.

**CAS guards.** `bd update --if-assignee/--if-status` provides compare-and-set
semantics — "only update if the current state matches what I expect." Prevents
race conditions where two agents act on stale reads.

**Gates.** `await_type` (gh:run, gh:pr, timer, human, mail) and `await_id` on
issues let a bead block on an external async condition, integrated into the
dependency graph. A gate-type issue doesn't need manual intervention to
unblock — it resolves when the external condition is met.

**Wisps.** Ephemeral molecules for throwaway operational work, excluded from
federation sync. Can be promoted (`bd mol squash`) or discarded
(`bd mol burn`).

**Compaction.** Tiered snapshots shrink old closed-issue text while preserving
restorable state. Solves a long-horizon problem: the database grows
indefinitely as work accumulates.

**Molecule progress view.** `bd mol progress` returns a resumability snapshot
(done/current/ready/blocked/pending) so an agent picking up mid-molecule
knows exactly where it left off.

## Adoptable now (HITL improvements)

These concepts improve yojana for solo human use without requiring
multi-agent infrastructure.

### Gates (blocked-on-external)

Yojana has `depends_on` edges for task-to-task blocking but nothing for
"blocked on CI" or "waiting on PR review." That state is implicit — the
operator knows it, but `yojana_ready` doesn't.

A lightweight implementation: add a `blocked_on` typed-ref field (or a
gate-type edge to a sentinel) that `yojana_ready` respects. Even a free-text
field that excludes the task from the ready set when non-empty would capture
the most common case. This makes the ready set more honest without structural
change.

### Arc progress / resumption view

Beads' molecule progress gives a single scan of "where did I leave off."
Yojana's arc phases + task statuses contain this information, but no query
surfaces it as a resumption-oriented summary.

A `yojana_arc_progress` query that computes done/ready/blocked breakdown
per phase — similar to what `yojana_context` shape=`agent` assembles but
as a standalone overview — would help both human resumption and future agent
dispatch.

### Queryable history

Yojana stores transition history as a JSON array on each task row. This works
for per-task audit but makes cross-task queries expensive ("tasks stuck in
in-progress for 3+ days", "what changed in the last hour"). Beads uses a
separate `events` table.

Migrating history to a dedicated table with indexed timestamps would enable
staleness detection, velocity metrics, and better debugging. The JSON blob
could remain as a denormalized cache for per-task reads if needed.

## Required for autonomous long-horizon work

If yojana were to support multiple autonomous agents, these are the
structural gaps roughly in priority order.

### 1. Assignee / agent identity

**Gap:** No assignee, owner, or agent-id field anywhere in the schema.
Cannot dispatch work to multiple agents without tracking who has what.

**Beads mechanism:** `assignee` field + `bd assign` command.

**Implementation:** Add column to `tasks` table. Low effort.

### 2. Claim / lease

**Gap:** Without atomic claim, two agents can grab the same ready task.
`yojana_ready` returns a set but nothing marks a task as taken.

**Beads mechanism:** Lease table with expiry and heartbeat. First-writer-wins
semantics on the ready-to-in-progress transition.

**Implementation:** Either a separate leases table or a compare-and-swap on
the status transition that also sets assignee. Medium effort. The single
SQLite writer actually makes atomicity easier than Dolt — no cell-merge
races — but the lease-expiry and heartbeat patterns are still needed to
handle agent crashes.

### 3. Heartbeat and reaper

**Gap:** An agent crash mid-task strands work in `in-progress` forever.

**Beads mechanism:** `bd reclaim` scans expired leases, reverts to `open`.

**Implementation:** Either a background timer in the systemd service or a
periodic MCP call. Medium effort.

### 4. Failure / retry state

**Gap:** `wontfix` means "decided not to do this," not "tried and failed."
No way to distinguish deliberate closure from failed execution that should
be retried.

**Beads mechanism:** Beads doesn't model this cleanly either — it relies on
lease expiry to handle crashes and manual reopen for failures. But the
lease-expiry path provides automatic recovery from the most common failure
mode (agent death).

**Implementation:** Add a `failed` status with transitions to
`ready-for-agent` (retry) and `wontfix` (give up). Pair with an optional
`failure_count` or `last_failure` field for retry budgeting.

### 5. Concurrent write support

**Gap:** SQLite's single-writer mutex serializes all MCP operations. Fine
for one human, potentially a bottleneck for many agents.

**Beads mechanism:** Dolt with cell-level merge + `row_lock` for ownership
operations.

**Implementation:** At low agent counts (< 10), SQLite serialization is
probably fine — the mutex wait is sub-millisecond for typical task
operations. Beyond that, options are WAL mode with BEGIN IMMEDIATE
transactions (already used), Dolt migration, or a connection-pool approach
with optimistic concurrency. Hard, and likely unnecessary until proven.

### 6. Gates (repeated from above)

Even more important for autonomous work than for HITL. Long-horizon agent
tasks constantly block on external events (CI, PR merge, deploy, approval).
Without gates, agents must poll or a human must manually unblock.

### 7. Structured multi-step workflows

**Gap:** Arcs are phase-centric (ordered lifecycle stages with auto-gating).
Beads molecules are task-graph-centric (parent issue with dependency-driven
child sequencing). These solve overlapping but different problems.

Arcs answer "what lifecycle phase is this project in?" Molecules answer
"given this decomposed task, what can I work on next?"

**Decision point:** Do arcs evolve to subsume molecule semantics (phases
become dependency-ordered rather than just sequentially ordered)? Or does a
parallel concept emerge? The answer depends on whether autonomous agents
would work within arc-structured projects or on standalone decomposed tasks.

## What's not worth adopting

**Wisps.** Exist because autonomous agents generate throwaway work that
shouldn't pollute the issue database. A solo human operator simply doesn't
create tasks they don't want.

**Compaction.** Solves database growth from long-lived autonomous operation.
Yojana's database is small enough that this is irrelevant for the
foreseeable future.

**Row lock.** Solves Dolt's cell-level merge semantics. SQLite's
single-writer model doesn't have this problem.

**Federation.** Beads syncs issue state across Dolt remotes for distributed
teams/rigs. Yojana is a local single-user service.
