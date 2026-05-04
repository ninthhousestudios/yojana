Status: needs-triage

# 01 — HTTP skeleton + project CRUD

## Parent

`.scratch/yojana-v0/PRD.md`

## What to build

Set up the yojana Rust crate with an HTTP server, SQLite database, and the first MCP tool. End-to-end: a client connects, creates a project, lists projects, gets a project by ID, updates a project. This proves the full stack (HTTP → MCP dispatch → Store → SQLite) works.

Follow the patterns established by sutra/smriti/sangha in the manas ecosystem: rmcp for MCP tool dispatch, rusqlite for SQLite, same project structure conventions.

- Cargo workspace at `manas/yojana/`
- Binary entry point: `yojana serve` starts HTTP server
- SQLite database at `~/.yojana/yojana.db` (created on first run)
- Migration 0001: projects table (id, slug, title, description, status, history, created_at, updated_at)
- Store module: project create, get, list, update
- `yojana_project` MCP tool dispatching by `action` parameter (create, get, list, update)
- UUID v7 for primary keys

## Acceptance criteria

- [ ] `cargo build` succeeds with no warnings
- [ ] `yojana serve` starts an HTTP server and accepts MCP connections
- [ ] `yojana_project action=create` creates a project with slug, title, description; returns the created project with UUID v7 id
- [ ] `yojana_project action=get` retrieves a project by id or slug
- [ ] `yojana_project action=list` returns all projects, filterable by status
- [ ] `yojana_project action=update` modifies title, description, or status; records change in history
- [ ] Slug uniqueness enforced at the DB level
- [ ] Project status defaults to `active`; valid values: active, paused, archived
- [ ] Store module has unit tests against in-memory SQLite covering CRUD and uniqueness constraint
- [ ] DB file created automatically at `~/.yojana/yojana.db` on first serve

## Blocked by

None — can start immediately.
