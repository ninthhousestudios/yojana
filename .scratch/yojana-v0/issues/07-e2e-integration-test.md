Status: needs-triage

# 07 — End-to-end integration test

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

A full-flow integration test that validates the entire yojana v0 stack working together. This is the "it works" gate: if this test passes, v0 is shippable.

Test flow:
1. Start yojana with an in-memory SQLite DB
2. Create a project ("yojana", "Yojana task graph server")
3. Create three tasks: A (no deps), B (depends_on A), C (depends_on B)
4. Verify ready shows only A
5. Transition A: needs-triage → ready-for-agent → in_progress → done
6. Verify ready now shows B
7. Fetch summary context for B — verify edge counts and status
8. Fetch working context for C — verify it shows B and A as neighbors
9. Add a conversation message to B
10. Fetch working context for B — verify conversation appears
11. Query by status=done — verify only A returned
12. Query across all projects — verify it works with one project

## Acceptance criteria

- [ ] Test runs against an in-memory SQLite DB (no filesystem side effects)
- [ ] All 12 steps pass
- [ ] Test is a single Rust integration test, runnable via `cargo test`
- [ ] No external dependencies (no HTTP server needed — tests the Store/Graph/Assembler layers directly)

## Blocked by

- 06 — Context shapes (summary + working)
