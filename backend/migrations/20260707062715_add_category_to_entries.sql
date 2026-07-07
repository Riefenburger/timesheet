-- Which pay category this session falls under. NULL = entered before a
-- rate existed (the "leave it blank" case), with a warning shown in the UI.
ALTER TABLE time_entries
    ADD COLUMN category TEXT
    CHECK (category IN ('teaching', 'assisting', 'office'));