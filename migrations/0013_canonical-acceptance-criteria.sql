-- 0012 normalized string elements to {text, done} and passed everything else
-- through untouched, so it left three gaps: objects with no "done" key, element
-- types the old `items: true` schema allowed (booleans, numbers, nulls, nested
-- arrays, objects with no "text"), and whole columns that are not a JSON array.
-- Rows made entirely of those were never selected at all, yet 0012 records as
-- applied and never revisits them.
--
-- After this migration every element is an object with a string "text" and a
-- boolean "done". Anything that cannot be read as a criterion is rewritten to a
-- visible <unmigratable> criterion rather than being dropped or left in place,
-- and the untouched original is preserved in the quarantine table first, so no
-- raw value is lost to the rewrite.
--
-- Wrapped in a transaction: the migration runner applies each file with
-- execute_batch and records it afterwards, so a failure partway through a
-- multi-statement file would otherwise leave the DB half-rewritten and the
-- migration unrecorded. The quarantine insert is also idempotent on its own.
BEGIN;

CREATE TABLE IF NOT EXISTS acceptance_criteria_quarantine (
  task_id      BLOB PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
  original     TEXT NOT NULL,
  quarantined_at INTEGER NOT NULL
);

-- Preserve the original of every row this migration cannot rewrite losslessly:
-- a column that is not a JSON array, or an array holding an element that is
-- neither a string nor an object with a string "text". A missing "done" is a
-- pure fill-in and loses nothing, so those rows are not quarantined.
--
-- A NULL column is excluded deliberately: it carries nothing to preserve, and
-- the tasks schema declares acceptance_criteria NOT NULL DEFAULT '[]' so it
-- cannot occur here anyway. Stating it in the predicate rather than letting
-- OR IGNORE swallow the resulting NOT NULL violation — the ignore is there for
-- the primary-key conflict on re-run, and should not be quietly doing more.
INSERT OR IGNORE INTO acceptance_criteria_quarantine (task_id, original, quarantined_at)
SELECT t.id, t.acceptance_criteria, CAST(strftime('%s','now') AS INTEGER) * 1000
FROM tasks t
WHERE t.acceptance_criteria IS NOT NULL
  AND (NOT json_valid(t.acceptance_criteria)
   OR json_type(t.acceptance_criteria) <> 'array'
   OR EXISTS (
        SELECT 1 FROM json_each(t.acceptance_criteria) je
        WHERE je.type NOT IN ('text', 'object')
           OR (je.type = 'object' AND json_type(je.value, '$.text') <> 'text')
           OR (je.type = 'object' AND json_type(je.value, '$.text') IS NULL)
      ));

-- Rewrite every element of every well-formed array to the canonical shape.
-- Element type comes from je.type, never json_type(je.value): for a string
-- element je.value is bare text and json_type() rejects it as malformed JSON.
-- "done" is true only for a real JSON true — a missing key, false, or anything
-- else reads as not done, deliberately rather than by coercion.
WITH normalized AS (
  SELECT
    t.id AS id,
    json_group_array(
      CASE
        WHEN je.type = 'text'
        THEN json_object('text', je.value, 'done', json('false'))
        WHEN je.type = 'object' AND json_type(je.value, '$.text') = 'text'
        THEN json_object(
               'text', json_extract(je.value, '$.text'),
               'done', CASE WHEN json_type(je.value, '$.done') = 'true'
                            THEN json('true') ELSE json('false') END
             )
        ELSE json_object(
               'text',
               '<unmigratable ' || je.type || ': '
                 || COALESCE(CAST(je.value AS TEXT), 'null') || '>',
               'done', json('false')
             )
      END
      ORDER BY je.key
    ) AS criteria
  FROM tasks t, json_each(t.acceptance_criteria) je
  WHERE json_valid(t.acceptance_criteria)
    AND json_type(t.acceptance_criteria) = 'array'
  GROUP BY t.id
)
UPDATE tasks
SET acceptance_criteria = normalized.criteria
FROM normalized
WHERE tasks.id = normalized.id
  AND tasks.acceptance_criteria <> normalized.criteria;

-- Columns that are not a JSON array at all cannot be walked element-wise. The
-- original is in quarantine; leave a visible criterion in its place so the
-- value neither disappears nor keeps rendering as unreadable forever.
UPDATE tasks
SET acceptance_criteria = json_array(
      json_object(
        'text', '<unmigratable column: ' || COALESCE(acceptance_criteria, 'null') || '>',
        'done', json('false')
      )
    )
WHERE acceptance_criteria IS NULL
   OR NOT json_valid(acceptance_criteria)
   OR json_type(acceptance_criteria) <> 'array';

COMMIT;
