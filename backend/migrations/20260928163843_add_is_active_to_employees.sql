-- Deactivation keeps an employee's history (entries, rates, typed totals) while
-- removing them from active rosters, payroll runs and portal access. Hard-delete
-- stays available only for employees created in error, guarded by the RESTRICT
-- foreign keys on time_entries / category_hours / period_dollar_totals.
ALTER TABLE employees ADD COLUMN is_active boolean NOT NULL DEFAULT true;
