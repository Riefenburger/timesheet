-- Stage 1 schema for phone (SMS) login via Twilio Verify.

-- One number = one account. Nullable, because the existing staff have none yet
-- and email/password remains the universal path. Postgres allows many NULLs
-- under a unique index, so this does not collide for them.
--
-- Always stored in E.164 (+15551234567) so the login lookup is an exact match;
-- normalisation happens in Rust (verify::normalize_phone) before any write or
-- lookup, and this CHECK is the backstop against anything unnormalised landing
-- in the column.
--
-- A number only reaches this column after Twilio Verify has approved it during
-- signup, so the column's presence IS the verification — EXCEPT when an admin
-- sets it by hand on the employee form, which is an admin assertion and is not
-- SMS-verified.
ALTER TABLE employees ADD COLUMN phone_number text UNIQUE;
ALTER TABLE employees ADD CONSTRAINT phone_number_is_e164
    CHECK (phone_number IS NULL OR phone_number ~ '^\+[1-9][0-9]{7,14}$');

-- Rate limiting for the verification endpoints. In a table rather than in
-- memory because Railway may run more than one backend instance, and an
-- in-memory counter would not be shared between them. Twilio Verify bills per
-- verification attempt, so the global ceiling enforced against this table is
-- the actual cost control.
CREATE TABLE verification_attempts (
    id           bigserial PRIMARY KEY,
    phone_number text,
    ip           text,
    kind         text NOT NULL CHECK (kind IN ('start', 'check')),
    succeeded    boolean NOT NULL DEFAULT false,
    created_at   timestamptz NOT NULL DEFAULT now()
);
-- Per-number limits and lockouts.
CREATE INDEX verification_attempts_phone_idx ON verification_attempts (phone_number, created_at);
-- Per-IP limits, and the global ceiling / cleanup sweep.
CREATE INDEX verification_attempts_created_idx ON verification_attempts (created_at);

-- Sessions now live up to 120 days and are refreshed on use, so expired rows
-- accumulate for far longer than before. This index serves the periodic
-- cleanup sweep.
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);
