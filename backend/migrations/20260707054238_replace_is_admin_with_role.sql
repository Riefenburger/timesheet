-- Add the new role column, defaulting everyone to 'user'.
ALTER TABLE employees
    ADD COLUMN role TEXT NOT NULL DEFAULT 'user'
    CHECK (role IN ('user', 'admin', 'super_admin'));

-- Carry over existing admins: anyone who was is_admin = true becomes 'admin'.
UPDATE employees SET role = 'admin' WHERE is_admin = true;

-- Drop the old boolean now that its data has been migrated.
ALTER TABLE employees DROP COLUMN is_admin;