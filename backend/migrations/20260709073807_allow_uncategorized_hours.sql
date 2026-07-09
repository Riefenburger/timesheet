-- Allow 'uncategorized' so no-rate/blank logged hours have an editable home.
ALTER TABLE category_hours
    DROP CONSTRAINT category_hours_category_check;

ALTER TABLE category_hours
    ADD CONSTRAINT category_hours_category_check
    CHECK (category IN ('teaching', 'assisting', 'office', 'uncategorized'));