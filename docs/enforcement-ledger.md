# Enforcement ledger — yojana

The append-only record of yojana's architectural constraints: what is governed,
why, at what severity, and how each rule was verified to bind. `.sutra/rules.toml`
is the machine-readable source of truth; this ledger is the human-readable
rationale and the maintenance history that `vidhi-sutra-tend` diffs against.

**Append-only.** Rows are added or their status is amended with a dated note;
rows are never silently deleted. A retired constraint is marked retired with a
rationale, not removed.

## Buckets

- **(a) graph-enforced** — expressible as a sutra constraint and actively bound
  (forbidden_dep, forbidden_external, confined_external, max_fan_in,
  forbidden_pattern). The guard denies or warns at edit time.
- **(b) deferred** — a constraint whose trigger structure does not exist yet;
  parked with a `# TRIGGER:` note until the anticipated code lands.
- **(c) prose invariant** — a rule stated in CLAUDE.md / design doc that the
  graph cannot express; enforcement is human review only.
- **(d) convention** — FCA-tracked pattern preference, lifecycle-managed.

## (a) Graph-enforced constraints

| id | name | kind | severity | scope / target | provenance | status |
|----|------|------|----------|----------------|------------|--------|
| 9f10049e | no-server-database | forbidden_external | blocking | all files | design doc § storage: SQLite, not Postgres | live |
| 8c3f8efb | sqlite-behind-db-layer | confined_external (rusqlite) | blocking | src/db.rs, src/error.rs | design doc § storage boundary | live |
| 5b696604 | no-cross-tier-clients | forbidden_external | blocking | all files | design doc § context_refs — resolution lives in manas-cli | live |
| fc34bfa9 | domain-must-not-import-tools | forbidden_dep | blocking | src/context.rs → src/tools/** | layering invariant — domain upstream of tools | live |
| d0470484 | task-tool-fan-in | max_fan_in (10) | advisory | src/tools/task.rs | fan-in watch on the widest tool handler | live |
| 2aa3cf1c | no-clone-driven-dev | forbidden_pattern (.clone()) | advisory | src/ | rust.toml — No Clone-Driven Development | live |
| e1e40c2a | no-to-owned-bypass | forbidden_pattern (.to_owned()) | advisory | src/ | rust.toml — No Clone-Driven Development | live |
| b5ca7cee | no-allow-attributes | forbidden_pattern (#[allow]) | blocking | src/ | rust.toml — lint-layer tamper resistance | live |
| 473054d5 | unsafe-requires-waiver | forbidden_pattern (unsafe {}) | advisory | src/ | rust.toml — unsafe carries a safety argument | live |
| b4d5f6b4 | no-todo-unimplemented | forbidden_pattern (todo!/unimplemented!) | blocking | src/ | rust.toml — no stubbed control flow committed | live |
| ba7dca02 | no-unwrap | forbidden_pattern (.unwrap()) | blocking | src/ | rust.toml — panics carry invariant messages | live |
| c464ae06 | serializer-no-io | forbidden_pattern (std::{fs,io,net,process}) | blocking | src/export/manifest.rs | design doc § purity — manifest is a pure serializer; writer.rs owns I/O | live |

**Notes**

- `no-clone-driven-dev` / `no-to-owned-bypass` are advisory by design: the
  house rule treats these as borrow-checker-bypass smells, but legitimate
  owned-value transfers (e.g. cloning a borrowed `String` field into an output
  struct) are common and correct. ~44 current matches, all such transfers;
  reviewed, not debt.
- `no-unwrap` / `no-todo-unimplemented` duplicate clippy lints already in
  `Cargo.toml [lints.clippy]`. They are blocking (not advisory) so the guard
  intercepts at edit time, which clippy structurally cannot; clippy stays at
  `warn` as the backstop for code outside sutra's scope. Test code
  (`#[cfg(test)]`) is excluded on both guard and review paths.

## (b) Deferred constraints

_None._ `no-cross-tier-clients` is a live tripwire rather than a deferred entry:
there are no sibling-crate deps today, but the rule binds now and denies any
future one.

## (c) Prose invariants (human-review only)

| invariant | source | enforcement |
|-----------|--------|-------------|
| Yojana validates ref *shape* but does not resolve refs; cross-tier resolution lives in manas-cli (principle 9). | design doc § context_refs | review — partially backed by no-cross-tier-clients (blocks the client deps resolution would require) |
| One SQLite DB per yojana root; local-first, no server DB. | design doc § storage | review — backed by no-server-database |

## (d) Conventions

FCA convention lifecycle is managed via `sutra_conventions`. No promotions or
deprecations recorded yet; first triage deferred (clone-noise already classified
as advisory above).

## Maintenance history

- **2026-08-06 — checkpoint:yojana/53** (first tend; export foundation review).
  Backfilled this ledger from existing `.sutra/rules.toml` provenance (repo was
  seeded with rules but no ledger). Health check: all 11 pre-existing
  constraints bind, zero `dead_constraint` warnings; only advisory
  `no-clone-driven-dev` violations, all reviewed as legitimate owned-value
  transfers. Added **c464ae06 serializer-no-io** (blocking) to lock the export
  serializer's purity boundary as its interior first appeared — verified to
  bind (1 file matched) and to bite (a probe `std::fs::metadata` in manifest.rs
  was denied at edit time). No drift found. Next tend: the records-layer slice
  landing (yojana/51 I4/I11/I13), when manifest.rs and the writer grow.
