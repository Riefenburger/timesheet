-- Phase 1 of splitting employees.name into parts.
--
-- The new columns are NULLABLE and `name` is KEPT, deliberately:
--   * The back-fill is done by hand through the admin UI, one employee at a
--     time, after this deploys. During that window some rows are split and some
--     are not, so every read falls back to `name` and the app stays correct
--     throughout.
--   * `name` is the only copy of the original string. Dropping it (or replacing
--     it with a generated column, which cannot be written) would destroy the
--     evidence on live payroll records if a split turned out wrong.
--
-- There is deliberately NO back-fill here: migrations run automatically on
-- deploy, and splitting real payroll names unreviewed is exactly what must not
-- happen. Phase 2 (renaming name -> name_legacy, never dropping it) is a
-- separate migration, only once the back-fill is complete and verified.
ALTER TABLE employees
    ADD COLUMN first_name     text,
    ADD COLUMN last_name      text,
    ADD COLUMN middle_initial text;

-- New employees are built from the parts, so the legacy single string is no
-- longer written for them.
ALTER TABLE employees ALTER COLUMN name DROP NOT NULL;

-- A row is either split or not, never half — so "first_name IS NULL" is an
-- unambiguous test for "still needs splitting".
ALTER TABLE employees ADD CONSTRAINT name_parts_paired
    CHECK ((first_name IS NULL) = (last_name IS NULL));

ALTER TABLE employees ADD CONSTRAINT middle_initial_is_one_letter
    CHECK (middle_initial IS NULL OR char_length(middle_initial) = 1);
