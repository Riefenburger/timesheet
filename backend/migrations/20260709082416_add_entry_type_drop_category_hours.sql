-- Each entry carries its pay type; totals are computed by summing entries.
ALTER TABLE time_entries
    ADD COLUMN type TEXT NOT NULL DEFAULT 'regular'
    CHECK (type IN ('regular', 'overtime', 'sick'));

-- category_hours is no longer needed: totals derive from entries, not stored rows.
DROP TABLE category_hours;