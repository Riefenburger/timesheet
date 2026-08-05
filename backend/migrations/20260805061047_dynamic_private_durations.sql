-- Dynamic private-session durations with global default rates. Replaces the
-- hardcoded 20/30/45/60 assumption. The super-admin manages these tiers; each
-- has a global default rate that pre-fills per-employee private rates.
CREATE TABLE private_durations (
    id BIGSERIAL PRIMARY KEY,
    duration_minutes INT NOT NULL UNIQUE CHECK (duration_minutes > 0),
    global_rate NUMERIC(10,2) NOT NULL DEFAULT 0 CHECK (global_rate >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed the real durations (20/30/60), dropping 45. Rates default to 0 —
-- the super-admin sets real values via the UI.
INSERT INTO private_durations (duration_minutes) VALUES (20), (30), (60);

-- Drop the hardcoded duration CHECKs; durations are now validated at the app
-- layer against private_durations (same pattern as text-based categories).
ALTER TABLE time_entries DROP CONSTRAINT IF EXISTS time_entries_session_duration_check;
ALTER TABLE category_hours DROP CONSTRAINT IF EXISTS category_hours_session_duration_check;