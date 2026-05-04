# Review Fixes Plan

Findings from the v0 code review, organized into waves by priority.

## Wave 1 — Fix Now (medium severity, correctness/safety)

### 1. Tag filter LIKE false positives

**File:** `src/db.rs` (line ~630)

**Problem:** `format!("%\"{tag}\"%")` used in a LIKE clause matches tags containing SQL wildcards (`%`, `_`). A tag like `"a%b"` would incorrectly match `"axb"`.

**Fix:** Replace LIKE-based filtering with SQLite's `json_each()`:
```sql
EXISTS (SELECT 1 FROM json_each(t.tags) WHERE json_each.value = ?N)
```
This gives exact JSON array membership testing. No ESCAPE hacks needed.

### 2. Cannot clear Optional task fields

**File:** `src/db.rs` (`update_task`, lines ~531-566)

**Problem:** The `Option<Option<String>>` pattern isn't used, so there's no way to distinguish "don't change this field" from "set it to NULL". Passing `Some("".into())` stores an empty string, not NULL.

**Fix:** Use a `FieldUpdate<T>` enum or the `Option<Option<T>>` (double-option) pattern:
- `None` → don't change
- `Some(None)` → set to NULL  
- `Some(Some(value))` → set to value

Apply to: `category`, `slice_type`, `implementation_plan`, `execution_record`, `reproduction`, `root_cause`.

Update `TaskUpdates` struct and the serde deserialization (use `#[serde(default, deserialize_with = "...")]` or a custom wrapper).

### 3. No project status validation

**File:** `src/db.rs` (`update_project`, line ~429)

**Problem:** Any string accepted as project status. A typo like `"actve"` is silently persisted.

**Fix:** Add a `VALID_PROJECT_STATUSES` constant (`active`, `paused`, `archived`) and validate in `update_project` before writing. Return `YojanaError::InvalidInput` on mismatch.

### 4. Duplicated neighbor-loading in context tool handler

**File:** `src/tools/context.rs` (lines 38-45 and 58-65)

**Problem:** The "working" and "review" match arms contain identical neighbor-fetching logic.

**Fix:** Extract to:
```rust
fn load_neighbors(db: &Db, task_id: Uuid, edges: &[EdgeRow]) -> Result<Vec<(TaskRow, Vec<EdgeRow>)>, YojanaError> {
    let nids = context::neighbor_ids(task_id, edges);
    let mut out = Vec::new();
    for nid in &nids {
        if let Some(ntask) = db.get_task(&nid.to_string())? {
            let nedges = db.list_edges_for_task(&ntask.id)?;
            out.push((ntask, nedges));
        }
    }
    Ok(out)
}
```

### 5. CancellationToken never triggered on shutdown

**File:** `src/main.rs` (~line 99-115)

**Problem:** The `CancellationToken` passed to `StreamableHttpServerConfig` is never cancelled after axum shuts down. In-flight MCP streaming sessions may not receive cleanup.

**Fix:** After `axum::serve(...).with_graceful_shutdown(shutdown_signal()).await`, call `cancel.cancel()`.

### 6. SIGTERM not handled

**File:** `src/main.rs` (`shutdown_signal`)

**Problem:** Only SIGINT (ctrl-c) triggers graceful shutdown. Process supervisors send SIGTERM.

**Fix:**
```rust
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = ctrl_c => {},
        _ = term.recv() => {},
    }
}
```

---

## Wave 2 — Harden (low severity, robustness)

### 7. Log warnings on JSON parse fallbacks

**Files:** `src/context.rs` (`last_history`, `json_array`), `src/db.rs` (`get_conversation_messages`)

**Problem:** `unwrap_or_default()` silently swallows corrupt JSON. Data loss is invisible.

**Fix:** Replace with `match` that logs `tracing::warn!("corrupt JSON in {field}: {err}")` before returning the default. No behavior change, just observability.

### 8. Prevent self-edges on non-dependency types

**File:** `src/db.rs` (`create_edge`)

**Problem:** `relates_to(A, A)` is allowed but semantically nonsensical.

**Fix:** Add early check: `if source_task_id == target_task_id { return Err(InvalidInput("self-edges not allowed")) }`.

### 9. Add missing state machine transitions

**File:** `src/state.rs`

**Missing transitions:**
- `needs-info → wontfix` (close without triaging)
- `in_progress → wontfix` (abandon mid-flight)
- `ready-for-agent → ready-for-human` (re-triage assignment)
- `ready-for-human → ready-for-agent` (re-triage assignment)

**Fix:** Add these to the `TRANSITIONS` map. Update tests.

### 10. Validate port env var with warning

**File:** `src/config.rs` (`parse_env_or`)

**Problem:** `YOJANA_PORT=abc` silently falls back to 4200.

**Fix:** Log `tracing::warn!("invalid {var}={val}, using default {default}")` on parse failure.

### 11. Port-binding TOCTOU race

**File:** `src/main.rs` (alive check)

**Problem:** `TcpStream::connect` probe has a race window before actual `bind`.

**Fix:** Remove the probe. Attempt `TcpListener::bind` directly. On `AddrInUse`, print the "already running" message and exit.

---

## Wave 3 — Scale prep (future, not urgent)

### 12. Migration versioning

**File:** `src/db.rs` (`run_migrations`)

**Problem:** All migrations run unconditionally on every startup. First ALTER TABLE migration will break.

**Fix:** Add a `_yojana_migrations` table tracking applied filenames/checksums. Check before executing. Standard pattern.

### 13. Scope edge loading for cycle check

**File:** `src/db.rs` (`create_edge` → `load_depends_on_edges`)

**Problem:** Loads ALL depends_on edges across all projects to check one cycle.

**Fix:** BFS/DFS from the target node backward through existing edges is sufficient — no need to load the full graph. Or scope to the project if cross-project depends_on is rare.

### 14. Scope ready detection to project

**File:** `src/db.rs` (`list_depends_on_with_status`)

**Problem:** Returns all depends_on edges system-wide even when querying one project.

**Fix:** Add an optional `project_id` parameter to filter at the SQL level. Add index on `(edge_type, source_task_id)`.

### 15. Pagination on list endpoints

**Files:** `src/db.rs` (`list_tasks`, `list_projects`)

**Problem:** Unbounded result sets.

**Fix:** Add `limit` and `offset` parameters to `TaskQueryFilter`. Default limit 100.

### 16. Sequence number safety under pooling

**File:** `src/db.rs` (`next_sequence_number`)

**Problem:** MAX+1 pattern is safe under single mutex but not under connection pooling.

**Fix:** Use `INSERT ... RETURNING sequence_number` with a trigger or use SQLite's `last_insert_rowid` with an autoincrement sequence table. Only needed if architecture moves to a pool.

### 17. Naming consistency: `in_progress` vs hyphens

**File:** `src/state.rs`

**Problem:** `in_progress` uses underscore while all other statuses use hyphens.

**Fix:** Rename to `in-progress`. This is a breaking change for any existing data — requires a migration to update task rows. Defer until there's a real dataset or do it before first production use.

---

## Execution notes

- Wave 1: do before first real use by another agent/tool
- Wave 2: do in next session or when the relevant code is touched
- Wave 3: defer until scale demands it or architecture changes

Tests required for each fix — don't just fix, add the regression test that proves the bug existed.
