-- How the employee is paid: 'check' or 'payroll'.
ALTER TABLE employees
    ADD COLUMN pay_method TEXT NOT NULL DEFAULT 'payroll'
    CHECK (pay_method IN ('check', 'payroll'));