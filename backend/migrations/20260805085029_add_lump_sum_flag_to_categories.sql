-- Lump-sum categories: the user checks "I did this" (adds a marker entry) and
-- gets a flat per-employee amount, which flows into the Other $ dollar bucket.
-- A category is now one of: normal (hours), private (duration/count), or
-- lump-sum (flat amount).
ALTER TABLE categories
    ADD COLUMN is_lump_sum BOOLEAN NOT NULL DEFAULT false;

-- A category can't be both private and lump-sum.
ALTER TABLE categories
    ADD CONSTRAINT categories_type_exclusive
    CHECK (NOT (is_private AND is_lump_sum));