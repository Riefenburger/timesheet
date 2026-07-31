-- Let category_hours also hold private session totals: a private row carries
-- a duration + count (and leaves the hour columns at 0); a normal row leaves
-- duration/count null. One table serves both.
ALTER TABLE category_hours
    ADD COLUMN session_duration INTEGER
        CHECK (session_duration IN (20, 30, 45, 60)),
    ADD COLUMN session_count INTEGER
        CHECK (session_count > 0);

-- Same both-or-neither guard as on time_entries.
ALTER TABLE category_hours
    ADD CONSTRAINT ch_private_fields_paired
    CHECK (
        (session_duration IS NULL AND session_count IS NULL)
        OR (session_duration IS NOT NULL AND session_count IS NOT NULL)
    );

-- Replace the unique constraint so it spans duration, with nulls treated as
-- equal — so a normal category still gets exactly one row per period, and
-- private gets one row per duration per period.
ALTER TABLE category_hours
    DROP CONSTRAINT category_hours_employee_id_period_start_period_end_category_key;

ALTER TABLE category_hours
    ADD CONSTRAINT category_hours_unique_key
    UNIQUE NULLS NOT DISTINCT (employee_id, period_start, period_end, category, session_duration);