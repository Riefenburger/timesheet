-- Marks a category_hours row as an admin override. When true, the stored
-- values are authoritative (the "edited" state). When a category has no
-- admin_edited row, its hours are computed live from entries (with overtime).
ALTER TABLE category_hours
    ADD COLUMN admin_edited BOOLEAN NOT NULL DEFAULT false;