-- Password hash for email+password login. Null until the employee completes
-- signup via an invite link (employees are created before they sign up).
ALTER TABLE employees
    ADD COLUMN password_hash TEXT;

-- Active login sessions. The token is stored in an httpOnly cookie; each
-- request looks it up here to identify the employee.
CREATE TABLE sessions (
    token TEXT PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX sessions_employee_id_idx ON sessions(employee_id);

-- Single-use, time-limited invites. A super-admin generates one for an existing
-- employee; visiting the link lets that person set their password. Consumed on use.
CREATE TABLE invites (
    token TEXT PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX invites_employee_id_idx ON invites(employee_id);