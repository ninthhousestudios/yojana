# Yojana

A task graph server for the manas ecosystem. Tracks projects, tasks, dependencies, and context shapes.

## Language

**Project**:
An organizational container for related work. Has a slug, status, and owns tasks and arcs.
_Avoid_: Workspace, team

**Task**:
The atomic unit of work. Has a status state machine, belongs to a project, optionally belongs to an arc phase.
_Avoid_: Issue, ticket, story

**Arc**:
A lifecycle container representing a feature, bugfix, spike, or initiative flowing through ordered phases. Lives within a project. Has its own status (active/paused/completed/abandoned) independent of phase progress.
_Avoid_: Epic, stream, initiative

**Phase**:
An ordered stage within an arc. Contains tasks. Has a name, slice type (AFK/HITL), gate (auto/manual), and status (pending/active/completed/skipped). The phase vocabulary is opinion-layer — defined by skills, not by yojana.
_Avoid_: Stage, step, milestone

**Edge**:
A typed, directed relationship between two tasks. Types: depends_on, blocks, relates_to, supersedes, refines, motivated_by.
_Avoid_: Link, relation

**Context shape**:
A bundled view of task or arc data optimized for a specific use case (summary, working, planning, agent).

## Relationships

- A **Project** contains zero or more **Arcs** and zero or more **Tasks**
- An **Arc** contains an ordered list of **Phases**
- A **Task** optionally belongs to one **Arc** and one **Phase** within it
- **Edges** connect **Tasks** to other **Tasks**, regardless of arc membership
- Arc-level dependencies are derived from **Edges** between their constituent **Tasks**

## Flagged ambiguities

- "Phase vocabulary" is opinion-layer (defined by skills at arc creation time), not grammar-layer (not enforced by yojana's schema). Yojana validates shape, not content.
