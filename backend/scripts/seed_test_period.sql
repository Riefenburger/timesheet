-- Local dev seed for eyeballing the payroll-timesheet export (work queue item #7).
--
-- Creates four throwaway employees named 'ZZ Test …' with entries in the
-- 2026-09-16 … 2026-09-30 period, covering the cases the export block layout has
-- to get right: a multi-rate employee (with overtime landing on the second rate),
-- an employee with two private durations, an employee with orphan hours in
-- categories they have no rate for, and a salaried employee.
--
-- LOCAL DATABASE ONLY — never run this against Railway. Run it from the repo root:
--   psql "$(grep -h '^DATABASE_URL' backend/.env | cut -d= -f2- | tr -d '"')" \
--        -f backend/scripts/seed_test_period.sql
--
-- Re-running is safe: it removes its own 'ZZ Test …' rows first. To delete the
-- test data and stop there, run just the cleanup block below.

BEGIN;

-- Guard: refuse to run anywhere that looks like production.
DO $guard$
DECLARE
    n  bigint;
    db text;
BEGIN
    SELECT current_database() INTO db;
    SELECT count(*) INTO n FROM employees;
    IF db <> 'timesheet' THEN
        RAISE EXCEPTION 'Refusing to seed: database is "%", expected the local "timesheet" DB.', db;
    END IF;
    IF n > 20 THEN
        RAISE EXCEPTION 'Refusing to seed: % employees looks like production, not local dev.', n;
    END IF;
END
$guard$;

-- --- Cleanup of any previous run (children first: these FKs are ON DELETE RESTRICT) ---
DELETE FROM time_entries
 WHERE employee_id IN (SELECT id FROM employees WHERE name LIKE 'ZZ Test%');
DELETE FROM category_hours
 WHERE employee_id IN (SELECT id FROM employees WHERE name LIKE 'ZZ Test%');
DELETE FROM period_dollar_totals
 WHERE employee_id IN (SELECT id FROM employees WHERE name LIKE 'ZZ Test%');
DELETE FROM employees WHERE name LIKE 'ZZ Test%';

-- --- Employees ---
INSERT INTO employees (name, employee_number, role, pay_method, pay_frequency, is_salaried, salary)
VALUES
    ('ZZ Test MultiRate', '901', 'user', 'payroll', 'bimonthly', false, NULL),
    ('ZZ Test Privates',  '902', 'user', 'payroll', 'bimonthly', false, NULL),
    ('ZZ Test Orphan',    '903', 'user', 'payroll', 'bimonthly', false, NULL),
    ('ZZ Test Salaried',  '904', 'user', 'payroll', 'bimonthly', true,  2000.00);

-- --- Rates ---
-- Insert order matters: the export's "Hourly" row is the lowest employee_rates.id,
-- so each rate goes in its own statement rather than a UNION whose order is not
-- guaranteed.
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'teaching', 25.00 FROM employees WHERE name = 'ZZ Test MultiRate';
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'office', 18.00 FROM employees WHERE name = 'ZZ Test MultiRate';
-- Lump-sum category: must NOT become a rate row; its dollars belong in Other $$.
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'social media', 25.00 FROM employees WHERE name = 'ZZ Test MultiRate';

INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'teaching', 26.00 FROM employees WHERE name = 'ZZ Test Privates';
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'private_30', 22.50 FROM employees WHERE name = 'ZZ Test Privates';
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'private_60', 40.00 FROM employees WHERE name = 'ZZ Test Privates';

INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'teaching', 24.00 FROM employees WHERE name = 'ZZ Test Orphan';
-- A rate with no hours this period: its row should render with blank hours cells.
INSERT INTO employee_rates (employee_id, label, amount)
SELECT id, 'office', 17.00 FROM employees WHERE name = 'ZZ Test Orphan';

-- --- Time entries ---
-- All dates fall in the Sun 2026-09-20 … Sat 2026-09-26 week, so the 40-hour
-- overtime line is crossed inside a single week and inside the pay period.

-- MultiRate: 36h teaching, then 8h office that straddles 40 (4 regular + 4 OT).
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-21', 'Ballet I',  'Studio A', 'seed', 12.00, 'teaching' FROM employees WHERE name = 'ZZ Test MultiRate';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-22', 'Ballet II', 'Studio A', 'seed', 12.00, 'teaching' FROM employees WHERE name = 'ZZ Test MultiRate';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-23', 'Ballet III','Studio A', 'seed', 12.00, 'teaching' FROM employees WHERE name = 'ZZ Test MultiRate';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-24', 'Front desk','Office',   'seed',  8.00, 'office'   FROM employees WHERE name = 'ZZ Test MultiRate';
-- Two lump-sum entries at 25.00 each => 50.00 of lump-sum dollars, no hours.
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-22', 'Reel',  'Office', 'seed', 0.00, 'social media' FROM employees WHERE name = 'ZZ Test MultiRate';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-23', 'Story', 'Office', 'seed', 0.00, 'social media' FROM employees WHERE name = 'ZZ Test MultiRate';

-- Privates: 10h teaching plus two private durations.
-- Private hours (3 x 30min = 1.50, 2 x 60min = 2.00) deliberately do NOT appear
-- in Regular any more — the private rows show session counts instead.
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-21', 'Jazz I', 'Studio B', 'seed', 10.00, 'teaching' FROM employees WHERE name = 'ZZ Test Privates';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category, session_duration, session_count)
SELECT id, '2026-09-22', 'Privates', 'Studio B', 'seed', 1.50, 'private', 30, 3 FROM employees WHERE name = 'ZZ Test Privates';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category, session_duration, session_count)
SELECT id, '2026-09-23', 'Privates', 'Studio B', 'seed', 2.00, 'private', 60, 2 FROM employees WHERE name = 'ZZ Test Privates';

-- Orphan: 6h teaching (has a rate), 5h assisting (no rate), 2h with no category
-- at all (arrives as 'uncategorized'). The last two must each get a
-- "(no rate)" row rather than disappearing.
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-21', 'Tap I', 'Studio C', 'seed', 6.00, 'teaching'  FROM employees WHERE name = 'ZZ Test Orphan';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-22', 'Tap II','Studio C', 'seed', 5.00, 'assisting' FROM employees WHERE name = 'ZZ Test Orphan';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-23', 'Cleanup','Studio C','seed', 2.00, NULL        FROM employees WHERE name = 'ZZ Test Orphan';

-- Salaried: has hours but no rates, so everything stays on the single Salary row.
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-21', 'Admin',  'Office', 'seed', 20.00, 'teaching' FROM employees WHERE name = 'ZZ Test Salaried';
INSERT INTO time_entries (employee_id, entry_date, class_name, teacher_room, details, hours, category)
SELECT id, '2026-09-22', 'Admin',  'Office', 'seed',  5.00, 'office'   FROM employees WHERE name = 'ZZ Test Salaried';

-- --- Admin-typed override, to exercise sick hours and the stored-row path ---
-- Same regular/OT the walk computes, plus 4 sick hours. Sick is employee-level on
-- the sheet, so it must appear on the block's FIRST row, not the office row.
INSERT INTO category_hours
    (employee_id, period_start, period_end, category, regular_hours, overtime_hours, sick_hours, admin_edited)
SELECT id, '2026-09-16', '2026-09-30', 'office', 4.00, 4.00, 4.00, true
FROM employees WHERE name = 'ZZ Test MultiRate';

-- --- Dollar buckets, to check the three dollar columns on the first row ---
-- Other $$ on the sheet is other_earn + lump-sum, i.e. 15.00 + 50.00 = 65.00.
INSERT INTO period_dollar_totals
    (employee_id, period_start, period_end, other_earn, competition_earn, coaching_earn)
SELECT id, '2026-09-16', '2026-09-30', 15.00, 30.00, 45.00
FROM employees WHERE name = 'ZZ Test MultiRate';

COMMIT;
