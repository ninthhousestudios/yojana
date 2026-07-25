-- The handoff field was a hand-maintained duplicate of the task graph that went
-- stale immediately and then actively misled: yojana_ready surfaced it ahead of
-- the ready list, so the first thing an agent read was often a checkpoint out of
-- date. The graph is the source of truth and does not drift. See yojana/41.
--
-- 0008 is kept in the migration list rather than deleted so that databases which
-- already recorded it stay consistent; on a fresh database 0008 and 0011 cancel.
ALTER TABLE projects DROP COLUMN handoff;
