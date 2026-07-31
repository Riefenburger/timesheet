-- Private sessions: logged as a count of fixed-duration sessions.
-- Both columns are null for normal entries, both set for private entries.
ALTER TABLE time_entries
    ADD COLUMN session_duration INTEGER
        CHECK (session_duration IN (20, 30, 45, 60)),
    ADD COLUMN session_count INTEGER
        CHECK (session_count > 0);

-- Guard: duration and count must be both-null (normal) or both-set (private).
-- Prevents a half-entered private record (duration without count, or vice versa).
ALTER TABLE time_entries
    ADD CONSTRAINT private_fields_paired
    CHECK (
        (session_duration IS NULL AND session_count IS NULL)
        OR (session_duration IS NOT NULL AND session_count IS NOT NULL)
    );