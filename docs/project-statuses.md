# Project statuses

## Where statuses are defined

`src/db.rs` line ~193:

```rust
const VALID_PROJECT_STATUSES: &[&str] = &["active", "production", "paused", "archived"];
```

This is the single source of truth. Adding a status here makes it valid for `update_project` and `yojana task-edit <project> --status <status>`.

## Where statuses are used

| Location | What it does |
|---|---|
| `src/db.rs` `update_project` | Validates new status against `VALID_PROJECT_STATUSES` |
| `src/main.rs` `Command::Projects` | Default listing shows "active" and "production" as separate sections; `--all` shows everything |
| `src/main.rs` `Command::TaskEdit` | Bare slug (no `/N`) routes to `update_project` |

## Adding a new status

1. Add the string to `VALID_PROJECT_STATUSES` in `src/db.rs`
2. If it should appear in the default `yojana projects` listing (without `--all`), add a section in the `Command::Projects` handler in `src/main.rs` (follow the pattern for "active" and "production")
3. Rebuild and install: `cargo install --path .`

## Current statuses

| Status | Meaning | Shown in default listing |
|---|---|---|
| `active` | Under active development | yes |
| `production` | Shipped, in production | yes |
| `paused` | On hold | no (use `--all`) |
| `archived` | No longer relevant | no (use `--all`) |
