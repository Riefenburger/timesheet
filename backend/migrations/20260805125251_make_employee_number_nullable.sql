-- Only payroll employees have an employee number; check-paid staff don't.
-- The UNIQUE constraint stays (Postgres allows multiple NULLs under UNIQUE).
ALTER TABLE employees ALTER COLUMN employee_number DROP NOT NULL;