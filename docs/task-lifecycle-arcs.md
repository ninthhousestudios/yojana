# yojana — task lifecycle arcs

Status: design-session prep
Date: 2026-05-08
Context: brainstorming session on karma + usage report analysis

---

## the problem

Yojana tracks tasks as flat nodes with edges. But real work flows through phases:

```
design → decompose → implement → review → fix → verify → done
```

Each phase spawns its own tasks — review findings become fix tasks, fix tasks get their own reviews. Today this structure is implicit (edge types, tags). Karma needs to understand phases to dispatch correctly, and humans need it to plan across streams.

## what's needed

A level above tasks — call it an **arc** (or stream, or epic). An arc represents a feature/initiative flowing through its lifecycle.

### arc properties

- **lifecycle state machine** — which phase the arc is in
- **phase-to-task mapping** — tasks belong to a phase within an arc
- **phase transitions** — when all review tasks are done, the arc advances to fix (or verify if clean)
- **dispatch hints** — each phase type has different dispatch characteristics:
  - design: HITL, needs human + grilling
  - decompose: HITL, produces task graph
  - implement: AFK, karma dispatches via vidhi
  - review: AFK, karma dispatches parallel lens agents
  - fix: AFK, implements review findings
  - verify: AFK, final test/clippy/fmt pass

### what this enables

- `yojana_query arc=chitta-personality-redesign` → everything grouped by phase
- karma reads arc state → dispatches appropriate agents for current phase
- "show me what's in review across all projects" → cross-project phase query
- natural handoff unit between humans and agents

## open questions

1. **Is an arc a project, a task, or a new entity?** Projects are too coarse (one project has many arcs). Tasks are too fine (an arc spans many tasks). A new entity with its own table and state machine seems right, but adds schema complexity.

2. **Phase vocabulary.** Is design/decompose/implement/review/fix/verify the right set? Should it be extensible? What about research phases, or spike phases?

3. **Phase transitions.** Automatic (all tasks in phase done → advance) or manual (human promotes)? Probably: auto-advance for AFK phases, manual for HITL phase gates.

4. **Backward movement.** Review finds a design flaw → arc moves back to design. How does this work? New tasks in the design phase, or a new "revision" of the arc?

5. **Relationship to existing edges.** Do arcs replace `motivated_by`/`refines` edges, or compose with them? Tasks within an arc still have dependency edges to each other.

6. **Cross-arc dependencies.** Arc A (sutra construction-site detection) blocks arc B (karma parallel dispatch heuristics). Do arcs have edges too?

## prior art in yojana

Yojana already has:
- Edge types: `depends_on`, `blocks`, `relates_to`, `supersedes`, `refines`, `motivated_by`
- Status state machine on tasks
- Nested sub-projects (workstreams)
- Context shapes

Sub-projects are the closest thing to arcs today, but they don't have lifecycle state machines.

## references

- This session's brainstorming on karma dispatch and task lifecycle
- Usage report friction: struct-field cascades, review→fix loops
- `yojana/docs/yojana-design.md` — current design
