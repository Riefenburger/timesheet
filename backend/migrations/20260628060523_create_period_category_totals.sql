CREATE TABLE period_category_totals (
    id               BIGSERIAL     PRIMARY KEY,
    employee_id      BIGINT        NOT NULL REFERENCES employees(id) ON DELETE RESTRICT,
    period_start     DATE          NOT NULL,
    period_end       DATE          NOT NULL,
    regular_hours    NUMERIC(6,2)  NOT NULL DEFAULT 0,
    overtime_hours   NUMERIC(6,2)  NOT NULL DEFAULT 0,
    other_earn       NUMERIC(10,2) NOT NULL DEFAULT 0,
    competition_earn NUMERIC(10,2) NOT NULL DEFAULT 0,
    coaching_earn    NUMERIC(10,2) NOT NULL DEFAULT 0,
    sick_hours       NUMERIC(6,2)  NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ   NOT NULL DEFAULT NOW(),

    UNIQUE (employee_id, period_start, period_end)
);