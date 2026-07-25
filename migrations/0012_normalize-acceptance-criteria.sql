-- acceptance_criteria was declared as an untyped JSON array, so two shapes
-- reached storage: a bare string per criterion, and a {text, done} object.
-- The CLI only ever parsed the object form. Backfill the string form to the
-- object form (order preserved, done defaulting to false) so one shape remains;
-- the write path now normalizes on ingest.
--
-- The aggregate runs in a CTE rather than a correlated subquery inside SET:
-- json_each() does not see the outer row when its argument is a column of the
-- table being updated, which silently yields an empty array.
--
-- Element type comes from je.type, not json_type(je.value): for a string
-- element je.value is the bare text, which json_type() rejects as malformed
-- JSON — the exact shape this migration exists to clean up.
WITH normalized AS (
  SELECT
    t.id AS id,
    json_group_array(
      CASE
        WHEN je.type = 'text'
        THEN json_object('text', je.value, 'done', json('false'))
        ELSE json(je.value)
      END
      ORDER BY je.key
    ) AS criteria
  FROM tasks t, json_each(t.acceptance_criteria) je
  WHERE json_valid(t.acceptance_criteria)
    AND json_type(t.acceptance_criteria) = 'array'
  GROUP BY t.id
  HAVING SUM(je.type = 'text') > 0
)
UPDATE tasks
SET acceptance_criteria = normalized.criteria
FROM normalized
WHERE tasks.id = normalized.id;
