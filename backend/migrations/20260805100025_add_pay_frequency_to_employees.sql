-- Per-employee pay frequency. Drives a view filter on the totals pages
-- (which employees show + the date-range type). Default 'bimonthly' matches
-- the existing semimonthly (1–15, 16–end) behavior.
ALTER TABLE employees
    ADD COLUMN pay_frequency TEXT NOT NULL DEFAULT 'bimonthly'
    CHECK (pay_frequency IN ('weekly', 'bimonthly', 'monthly'));