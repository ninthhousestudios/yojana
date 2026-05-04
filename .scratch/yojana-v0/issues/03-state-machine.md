Status: done

# 03 — State machine

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Add status transition validation and automatic history recording when task status changes. The state machine enforces which transitions are legal and rejects invalid ones.

Legal transitions:
```
needs-triage → needs-info, ready-for-agent, ready-for-human, wontfix
needs-info → needs-triage, ready-for-agent, ready-for-human, wontfix
ready-for-agent → in_progress, needs-triage
ready-for-human → in_progress, needs-triage
in_progress → done, needs-triage
done → (terminal)
wontfix → needs-triage (reopen)
```

- State machine module: pure function `validate_transition(from, to) → Result`
- History entry generation: `{ts, kind: "status_change", payload: {from, to}}`
- Integrated into `yojana_task action=update` — status changes go through the state machine
- Non-status updates bypass the state machine

## Acceptance criteria

- [ ] Valid transitions succeed and record a history entry with timestamp, from-status, and to-status
- [ ] Invalid transitions return a clear error naming the current status and the attempted target
- [ ] History entries append (never replace) in the tasks.history JSON array
- [ ] State machine is a pure module with no DB dependency, tested with unit tests covering all valid and invalid transitions
- [ ] `done` is terminal — no transitions out except back to `needs-triage` (reopen)

## Blocked by

- 02 — Task CRUD + sequence numbers
