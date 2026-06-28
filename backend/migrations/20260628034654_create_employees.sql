CREATE TABLE employees (
    id              BIGSERIAL    PRIMARY KEY,
    name            TEXT         NOT NULL,
    employee_number TEXT         NOT NULL UNIQUE,
    email           TEXT         UNIQUE,
    google_sub      TEXT         UNIQUE,
    is_admin        BOOLEAN      NOT NULL DEFAULT FALSE,
    is_salaried     BOOLEAN      NOT NULL DEFAULT FALSE,
    salary          NUMERIC(10,2),
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    CONSTRAINT salary_matches_pay_type
        CHECK (is_salaried = (salary IS NOT NULL))
);